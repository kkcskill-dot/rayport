use super::*;
use std::sync::Arc;
use tempfile::TempDir;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct State {
    count: u64,
    names: Vec<String>,
}

fn initial() -> State {
    State {
        count: 0,
        names: vec!["레이포트".to_owned()],
    }
}

fn increment(store: &SnapshotStore<State>) {
    store
        .transaction(|state| {
            state.count += 1;
            Ok(())
        })
        .unwrap();
}

#[test]
fn memory_rejection_rolls_back_and_return_values_are_preserved() {
    let store = SnapshotStore::memory(initial());
    let rejected: Result<(), _> = store.transaction(|state| {
        state.count = 99;
        state.names.clear();
        Err(StoreError::Rejected("policy rejected".to_owned()))
    });
    assert!(matches!(rejected, Err(StoreError::Rejected(_))));
    assert_eq!(store.read().unwrap(), initial());
    let result = store
        .transaction(|state| {
            state.count = 1;
            Ok("committed")
        })
        .unwrap();
    assert_eq!(result, "committed");
    assert_eq!(store.read().unwrap().count, 1);
}

#[test]
fn file_state_survives_reopen_and_initial_never_overwrites_it() {
    let directory = TempDir::new().unwrap();
    {
        let store = SnapshotStore::open(directory.path(), initial()).unwrap();
        increment(&store);
        assert!(directory.path().join(SNAPSHOT_NAME).is_file());
    }
    let mut replacement = initial();
    replacement.count = 999;
    let reopened = SnapshotStore::open(directory.path(), replacement).unwrap();
    assert_eq!(reopened.read().unwrap().count, 1);
    assert_eq!(reopened.read().unwrap().names, initial().names);
}

#[test]
fn rejected_persistent_mutation_does_not_change_snapshot() {
    let directory = TempDir::new().unwrap();
    let store = SnapshotStore::open(directory.path(), initial()).unwrap();
    let original = fs::read(directory.path().join(SNAPSHOT_NAME)).unwrap();
    let result: Result<(), _> = store.transaction(|state| {
        state.count = 5;
        Err(StoreError::Rejected("denied".to_owned()))
    });
    assert!(result.is_err());
    assert_eq!(
        fs::read(directory.path().join(SNAPSHOT_NAME)).unwrap(),
        original
    );
    assert_eq!(store.read().unwrap(), initial());
}

#[test]
fn second_open_is_rejected_until_first_store_is_dropped() {
    let directory = TempDir::new().unwrap();
    let first = SnapshotStore::open(directory.path(), initial()).unwrap();
    assert!(matches!(
        SnapshotStore::open(directory.path(), initial()),
        Err(StoreError::Unavailable(_))
    ));
    drop(first);
    assert!(directory.path().join(LOCK_NAME).exists());
    assert!(SnapshotStore::open(directory.path(), initial()).is_ok());
}

#[test]
fn checksum_mismatch_is_rejected_without_resetting_state() {
    let directory = TempDir::new().unwrap();
    drop(SnapshotStore::open(directory.path(), initial()).unwrap());
    let path = directory.path().join(SNAPSHOT_NAME);
    let mut envelope: Envelope = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    envelope.payload = "{}".to_owned();
    let corrupted = serde_json::to_vec(&envelope).unwrap();
    fs::write(&path, &corrupted).unwrap();
    assert!(matches!(
        SnapshotStore::open(directory.path(), initial()),
        Err(StoreError::Unavailable(_))
    ));
    assert_eq!(fs::read(&path).unwrap(), corrupted);
}

#[test]
fn unknown_schema_and_malformed_snapshots_are_not_initialized_over() {
    let directory = TempDir::new().unwrap();
    drop(SnapshotStore::open(directory.path(), initial()).unwrap());
    let path = directory.path().join(SNAPSHOT_NAME);
    let mut envelope: Envelope = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    envelope.schema = 2;
    fs::write(&path, serde_json::to_vec(&envelope).unwrap()).unwrap();
    assert!(SnapshotStore::open(directory.path(), initial()).is_err());
    fs::write(&path, "malformed data").unwrap();
    assert!(SnapshotStore::open(directory.path(), initial()).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "malformed data");
}

#[test]
fn memory_transactions_are_serialized_without_lost_updates() {
    let store = Arc::new(SnapshotStore::memory(initial()));
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let store = Arc::clone(&store);
            std::thread::spawn(move || {
                for _ in 0..100 {
                    increment(&store);
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    assert_eq!(store.read().unwrap().count, 800);
}

#[test]
fn concurrent_persistent_transactions_match_reopened_snapshot() {
    let directory = TempDir::new().unwrap();
    let store = Arc::new(SnapshotStore::open(directory.path(), initial()).unwrap());
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let store = Arc::clone(&store);
            std::thread::spawn(move || {
                for _ in 0..5 {
                    increment(&store);
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    assert_eq!(store.read().unwrap().count, 20);
    drop(store);
    assert_eq!(
        SnapshotStore::open(directory.path(), initial())
            .unwrap()
            .read()
            .unwrap()
            .count,
        20
    );
}

#[test]
fn failure_before_rename_keeps_previous_memory_and_disk_state() {
    let directory = TempDir::new().unwrap();
    let store = SnapshotStore::open(directory.path(), initial()).unwrap();
    store
        .inner
        .lock()
        .unwrap()
        .backend
        .as_mut()
        .unwrap()
        .fail_next_commit = Some(FailurePoint::BeforeRename);
    assert!(store
        .transaction(|state| {
            state.count = 8;
            Ok(())
        })
        .is_err());
    assert_eq!(store.read().unwrap().count, 0);
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
    increment(&store);
    drop(store);
    assert_eq!(
        SnapshotStore::open(directory.path(), initial())
            .unwrap()
            .read()
            .unwrap()
            .count,
        1
    );
}

#[test]
fn failure_after_rename_closes_store_until_recovery() {
    let directory = TempDir::new().unwrap();
    let store = SnapshotStore::open(directory.path(), initial()).unwrap();
    store
        .inner
        .lock()
        .unwrap()
        .backend
        .as_mut()
        .unwrap()
        .fail_next_commit = Some(FailurePoint::AfterRename);
    assert!(store
        .transaction(|state| {
            state.count = 9;
            Ok(())
        })
        .is_err());
    assert!(store.read().is_err());
    assert!(store.transaction(|_| Ok(())).is_err());
    drop(store);
    let recovered = SnapshotStore::open(directory.path(), initial()).unwrap();
    assert_eq!(recovered.read().unwrap().count, 9);
}

#[test]
fn panicking_update_poisoning_prevents_inconsistent_access() {
    let store = SnapshotStore::memory(initial());
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: Result<(), StoreError> = store.transaction(|state| {
            state.count = 12;
            panic!("injected application panic");
        });
    }));
    assert!(store.read().is_err());
    assert!(store.transaction(|_| Ok(())).is_err());
}

#[cfg(unix)]
#[test]
fn symlink_snapshot_and_lock_are_rejected_without_touching_target() {
    use std::os::unix::fs::symlink;
    let directory = TempDir::new().unwrap();
    let target = directory.path().join("outside-file");
    fs::write(&target, "unchanged").unwrap();
    let snapshot_dir = directory.path().join("snapshot-store");
    fs::create_dir(&snapshot_dir).unwrap();
    symlink(&target, snapshot_dir.join(SNAPSHOT_NAME)).unwrap();
    assert!(SnapshotStore::open(&snapshot_dir, initial()).is_err());
    let lock_dir = directory.path().join("lock-store");
    fs::create_dir(&lock_dir).unwrap();
    symlink(&target, lock_dir.join(LOCK_NAME)).unwrap();
    assert!(SnapshotStore::open(&lock_dir, initial()).is_err());
    assert_eq!(fs::read_to_string(&target).unwrap(), "unchanged");
}

#[cfg(unix)]
#[test]
fn symlink_directory_is_rejected_and_private_modes_are_enforced() {
    use std::os::unix::fs::symlink;
    let directory = TempDir::new().unwrap();
    let real = directory.path().join("real");
    fs::create_dir(&real).unwrap();
    let link = directory.path().join("linked");
    symlink(&real, &link).unwrap();
    assert!(SnapshotStore::open(&link, initial()).is_err());
    let store = SnapshotStore::open(&real, initial()).unwrap();
    assert_eq!(
        fs::metadata(&real).unwrap().permissions().mode() & 0o777,
        0o700
    );
    for name in [SNAPSHOT_NAME, LOCK_NAME] {
        assert_eq!(
            fs::metadata(real.join(name)).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    drop(store);
}

#[cfg(unix)]
#[test]
fn hard_linked_snapshot_is_rejected() {
    let directory = TempDir::new().unwrap();
    let storage = directory.path().join("store");
    drop(SnapshotStore::open(&storage, initial()).unwrap());
    fs::hard_link(
        storage.join(SNAPSHOT_NAME),
        directory.path().join("linked-copy"),
    )
    .unwrap();
    assert!(SnapshotStore::open(&storage, initial()).is_err());
}
