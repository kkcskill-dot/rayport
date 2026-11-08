//! Single-node, whole-state persistence for the initial Rayport release.
//!
//! This is a bounded metadata store, not an OLAP database or a distributed store.
//! Use one cooperating process on a local Unix filesystem with atomic rename,
//! working advisory locks, and file/directory fsync support. Network filesystems,
//! shared-volume multi-node deployments, and external edits while open are not
//! supported. The private directory must be owned and controlled by the service
//! account. SHA256 detects accidental corruption, not malicious rewriting.
//!
//! Each transaction clones state, validates it through the caller's closure,
//! then writes a private temporary file, fsyncs it, renames it, and fsyncs the
//! directory. Memory commits only after that sequence succeeds. An error after
//! rename leaves durability uncertain: both reads and writes then fail closed
//! until the store is dropped and explicitly reopened. A crash may leave
//! orphaned temporary files; they are never treated as authoritative state.

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};

const SNAPSHOT_NAME: &str = "snapshot.json";
const LOCK_NAME: &str = ".rayport.lock";
const SCHEMA_VERSION: u32 = 1;
const MAX_SNAPSHOT_BYTES: u64 = 64 * 1024 * 1024;

/// Stable error categories. Infrastructure errors intentionally omit paths,
/// serialized records, OS error messages, and other potentially sensitive data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    Rejected(String),
    Unavailable(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected(message) | Self::Unavailable(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for StoreError {}

fn unavailable(message: &str) -> StoreError {
    StoreError::Unavailable(message.to_owned())
}

fn checksum(payload: &[u8]) -> String {
    Sha256::digest(payload)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Thread-safe snapshots. Share one instance with `Arc`; do not independently
/// open the same persistent directory in each request or worker.
pub struct SnapshotStore<T> {
    inner: Mutex<StoreInner<T>>,
}

struct StoreInner<T> {
    value: T,
    backend: Option<FileBackend>,
    commit_uncertain: bool,
}

struct FileBackend {
    directory: PathBuf,
    directory_handle: File,
    // The open file owns the advisory lock. Never unlink the lock file on drop.
    _lock: File,
    #[cfg(test)]
    fail_next_commit: Option<FailurePoint>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    schema: u32,
    sha256: String,
    // Exact serialized bytes are preserved for deterministic checksum checking.
    payload: String,
}

struct PersistFailure {
    error: StoreError,
    commit_uncertain: bool,
}

impl From<StoreError> for PersistFailure {
    fn from(error: StoreError) -> Self {
        Self {
            error,
            commit_uncertain: false,
        }
    }
}

impl<T: Clone + Serialize + DeserializeOwned> SnapshotStore<T> {
    pub fn memory(initial: T) -> Self {
        Self {
            inner: Mutex::new(StoreInner {
                value: initial,
                backend: None,
                commit_uncertain: false,
            }),
        }
    }

    /// Open or initialize the private persistent directory. A corrupt snapshot
    /// or unknown envelope version is an error, never a reason to reset state.
    pub fn open(directory: &Path, initial: T) -> Result<Self, StoreError> {
        let mut backend = FileBackend::open(directory)?;
        let value = match backend.load()? {
            Some(value) => value,
            None => {
                backend.persist(&initial).map_err(|error| error.error)?;
                initial
            }
        };
        Ok(Self {
            inner: Mutex::new(StoreInner {
                value,
                backend: Some(backend),
                commit_uncertain: false,
            }),
        })
    }

    pub fn read(&self) -> Result<T, StoreError> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| unavailable("Storage is unavailable after an interrupted operation"))?;
        if inner.commit_uncertain {
            return Err(unavailable(
                "Storage requires recovery before further access",
            ));
        }
        Ok(inner.value.clone())
    }

    /// A rejected closure does not change memory or disk. Callers should keep
    /// the closure short and must not perform irreversible external side effects
    /// inside it: persistence can still fail after the closure succeeds.
    pub fn transaction<R>(
        &self,
        update: impl FnOnce(&mut T) -> Result<R, StoreError>,
    ) -> Result<R, StoreError> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| unavailable("Storage is unavailable after an interrupted operation"))?;
        if inner.commit_uncertain {
            return Err(unavailable(
                "Storage requires recovery before further access",
            ));
        }
        let mut proposed = inner.value.clone();
        let result = update(&mut proposed)?;
        if let Some(backend) = inner.backend.as_mut() {
            if let Err(failure) = backend.persist(&proposed) {
                inner.commit_uncertain = failure.commit_uncertain;
                return Err(failure.error);
            }
        }
        inner.value = proposed;
        Ok(result)
    }
}

impl FileBackend {
    #[cfg(unix)]
    fn open(directory: &Path) -> Result<Self, StoreError> {
        if let Ok(metadata) = fs::symlink_metadata(directory) {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(unavailable("Storage directory must be a real directory"));
            }
        }
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(directory)
            .map_err(|_| unavailable("Cannot create storage directory"))?;
        let directory_handle = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY | libc::O_CLOEXEC)
            .open(directory)
            .map_err(|_| unavailable("Cannot open storage directory"))?;
        directory_handle
            .set_permissions(fs::Permissions::from_mode(0o700))
            .map_err(|_| unavailable("Cannot secure storage directory"))?;
        let lock = open_private_regular(&directory.join(LOCK_NAME), true)?;
        fs2::FileExt::try_lock_exclusive(&lock)
            .map_err(|_| unavailable("Storage is already in use or cannot be locked"))?;
        Ok(Self {
            directory: directory.to_path_buf(),
            directory_handle,
            _lock: lock,
            #[cfg(test)]
            fail_next_commit: None,
        })
    }

    #[cfg(not(unix))]
    fn open(_directory: &Path) -> Result<Self, StoreError> {
        Err(unavailable(
            "Persistent storage requires a supported Unix filesystem",
        ))
    }

    fn load<T: DeserializeOwned>(&self) -> Result<Option<T>, StoreError> {
        let path = self.directory.join(SNAPSHOT_NAME);
        match fs::symlink_metadata(&path) {
            Ok(_) => (),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(unavailable("Cannot inspect storage snapshot")),
        }
        let file = open_private_regular(&path, false)?;
        let mut bytes = Vec::new();
        file.take(MAX_SNAPSHOT_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| unavailable("Cannot read storage snapshot"))?;
        if bytes.len() as u64 > MAX_SNAPSHOT_BYTES {
            return Err(unavailable("Storage snapshot exceeds the supported size"));
        }
        let envelope: Envelope = serde_json::from_slice(&bytes)
            .map_err(|_| unavailable("Storage snapshot is malformed"))?;
        if envelope.schema != SCHEMA_VERSION {
            return Err(unavailable("Storage snapshot schema is unsupported"));
        }
        let actual = checksum(envelope.payload.as_bytes());
        if actual != envelope.sha256 {
            return Err(unavailable("Storage snapshot checksum does not match"));
        }
        let value = serde_json::from_str(&envelope.payload)
            .map_err(|_| unavailable("Storage snapshot payload is incompatible"))?;
        Ok(Some(value))
    }

    fn persist<T: Serialize>(&mut self, value: &T) -> Result<(), PersistFailure> {
        let payload = serde_json::to_string(value)
            .map_err(|_| unavailable("Cannot serialize storage state"))?;
        let envelope = Envelope {
            schema: SCHEMA_VERSION,
            sha256: checksum(payload.as_bytes()),
            payload,
        };
        let bytes = serde_json::to_vec(&envelope)
            .map_err(|_| unavailable("Cannot serialize storage snapshot"))?;
        if bytes.len() as u64 > MAX_SNAPSHOT_BYTES {
            return Err(unavailable("Storage snapshot exceeds the supported size").into());
        }
        let destination = self.directory.join(SNAPSHOT_NAME);
        ensure_regular_if_present(&destination)?;
        let (mut temporary, temporary_path) = create_private_temp(&self.directory)?;
        let cleanup = TemporaryCleanup(temporary_path);
        temporary
            .write_all(&bytes)
            .and_then(|_| temporary.sync_all())
            .map_err(|_| unavailable("Cannot persist storage snapshot"))?;
        #[cfg(test)]
        if self.fail_next_commit == Some(FailurePoint::BeforeRename) {
            self.fail_next_commit = None;
            return Err(unavailable("Injected pre-commit persistence failure").into());
        }
        fs::rename(&cleanup.0, &destination)
            .map_err(|_| unavailable("Cannot replace storage snapshot"))?;
        #[cfg(test)]
        if self.fail_next_commit == Some(FailurePoint::AfterRename) {
            self.fail_next_commit = None;
            return Err(PersistFailure {
                error: unavailable("Storage commit is uncertain; recovery is required"),
                commit_uncertain: true,
            });
        }
        self.directory_handle
            .sync_all()
            .map_err(|_| PersistFailure {
                error: unavailable("Storage commit is uncertain; recovery is required"),
                commit_uncertain: true,
            })?;
        Ok(())
    }
}

#[cfg(unix)]
fn open_private_regular(path: &Path, create: bool) -> Result<File, StoreError> {
    ensure_regular_if_present(path)?;
    let file = OpenOptions::new()
        .read(true)
        .write(create)
        .create(create)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| unavailable("Cannot open a private storage file"))?;
    let metadata = file
        .metadata()
        .map_err(|_| unavailable("Cannot inspect a private storage file"))?;
    if !metadata.is_file() || metadata.nlink() != 1 {
        return Err(unavailable("Storage files must be unlinked regular files"));
    }
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|_| unavailable("Cannot secure a private storage file"))?;
    Ok(file)
}

#[cfg(not(unix))]
fn open_private_regular(_path: &Path, _create: bool) -> Result<File, StoreError> {
    Err(unavailable(
        "Persistent storage requires a supported Unix filesystem",
    ))
}

fn ensure_regular_if_present(path: &Path) -> Result<(), StoreError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => Err(
            unavailable("Storage files must be regular files, without symlinks"),
        ),
        #[cfg(unix)]
        Ok(metadata) if metadata.nlink() != 1 => Err(unavailable(
            "Storage files must not have additional hard links",
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(unavailable("Cannot inspect a storage file")),
    }
}

#[cfg(unix)]
fn create_private_temp(directory: &Path) -> Result<(File, PathBuf), StoreError> {
    for _ in 0..8 {
        let mut nonce = [0_u8; 16];
        File::open("/dev/urandom")
            .and_then(|mut random| random.read_exact(&mut nonce))
            .map_err(|_| unavailable("Cannot obtain randomness for private storage"))?;
        let name = nonce
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let path = directory.join(format!(".snapshot-{name}.tmp"));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&path)
        {
            Ok(file) => return Ok((file, path)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(unavailable("Cannot create temporary storage snapshot")),
        }
    }
    Err(unavailable("Cannot allocate a unique storage snapshot"))
}

#[cfg(not(unix))]
fn create_private_temp(_directory: &Path) -> Result<(File, PathBuf), StoreError> {
    Err(unavailable(
        "Persistent storage requires a supported Unix filesystem",
    ))
}

struct TemporaryCleanup(PathBuf);

impl Drop for TemporaryCleanup {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum FailurePoint {
    BeforeRename,
    AfterRename,
}

#[cfg(test)]
#[path = "CratesRayportAdaptersSrcPersistenceSnapshotStoreTests.rs"]
mod tests;
