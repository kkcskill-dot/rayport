use rayport_domain::{valid_identifier, Principal, Role};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct User {
    pub principal: Principal,
    pub token_sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApicConfig {
    pub execute_url: String,
    pub cancel_url: String,
    pub status_url: String,
    pub token_env: String,
    pub timeout_seconds: u64,
    pub contract_version: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub bind: String,
    pub public_origin: String,
    pub secure_cookie: bool,
    pub data_directory: PathBuf,
    pub web_directory: PathBuf,
    pub tenant_id: String,
    pub users: Vec<User>,
    pub apic: Option<ApicConfig>,
}
pub fn token_hash(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}
impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if self.bind.parse::<std::net::SocketAddr>().is_err() {
            return Err("bind must be an IP socket address".into());
        }
        let origin =
            reqwest::Url::parse(&self.public_origin).map_err(|_| "invalid public_origin")?;
        if !matches!(origin.scheme(), "http" | "https")
            || origin.host_str().is_none()
            || origin.path() != "/"
            || origin.query().is_some()
            || origin.fragment().is_some()
            || !origin.username().is_empty()
            || origin.password().is_some()
        {
            return Err("public_origin must be an exact http(s) origin".into());
        }
        if origin.origin().ascii_serialization() != self.public_origin {
            return Err("public_origin must be canonical: no trailing slash, default port, or uppercase hostname".into());
        }
        if origin.scheme() == "https" && !self.secure_cookie {
            return Err("https origin requires secure_cookie".into());
        }
        if origin.scheme() == "http"
            && !self
                .bind
                .parse::<std::net::SocketAddr>()
                .map_err(|_| "invalid bind")?
                .ip()
                .is_loopback()
        {
            return Err("HTTP public_origin requires a loopback bind".into());
        }
        if origin.scheme() == "http"
            && !matches!(origin.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))
        {
            return Err("non-loopback deployments require HTTPS public_origin".into());
        }
        if !valid_identifier(&self.tenant_id)
            || self.users.is_empty()
            || self.users.len() > 500
            || !self.users.iter().any(|u| u.principal.role == Role::Admin)
        {
            return Err("valid tenant and administrator required".into());
        }
        let mut ids = std::collections::HashSet::new();
        let mut hashes = std::collections::HashSet::new();
        for u in &self.users {
            if !valid_identifier(&u.principal.id)
                || u.principal.tenant_id != self.tenant_id
                || !ids.insert(&u.principal.id)
                || !hashes.insert(&u.token_sha256)
                || u.token_sha256.len() != 64
                || !u
                    .token_sha256
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && (!b.is_ascii_uppercase()))
                || u.principal.store_ids.len() > 1000
                || u.principal.store_ids.iter().any(|id| !valid_identifier(id))
            {
                return Err("invalid or duplicate user configuration".into());
            }
        }
        if let Some(a) = &self.apic {
            if a.contract_version != "rayport-apic-v1"
                || !(1..=30).contains(&a.timeout_seconds)
                || a.token_env.is_empty()
            {
                return Err("invalid APIC integration contract".into());
            }
            for address in [&a.execute_url, &a.cancel_url, &a.status_url] {
                let url = reqwest::Url::parse(address).map_err(|_| "invalid APIC URL")?;
                if url.scheme() != "https"
                    || url.host_str().is_none()
                    || !url.username().is_empty()
                    || url.password().is_some()
                    || url.fragment().is_some()
                    || url.query().is_some()
                {
                    return Err(
                        "APIC endpoints require HTTPS without embedded credentials/query/fragment"
                            .into(),
                    );
                }
            }
        }
        Ok(())
    }
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = fs::read(path).map_err(|_| "cannot read config")?;
        if bytes.len() > 1_048_576 {
            return Err("config too large".into());
        }
        let c: Self = serde_json::from_slice(&bytes).map_err(|_| "invalid config JSON")?;
        c.validate()?;
        Ok(c)
    }
}

/// Creates an operator-owned config and separate first-login token. Never prints a credential.
pub fn initialize(directory: &Path) -> Result<(), String> {
    fs::create_dir_all(directory).map_err(|_| "cannot create installation directory")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
            .map_err(|_| "cannot restrict directory")?;
    }
    let directory = fs::canonicalize(directory).map_err(|_| "cannot resolve directory")?;
    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let c = Config {
        bind: "127.0.0.1:8080".into(),
        public_origin: "http://127.0.0.1:8080".into(),
        secure_cookie: false,
        data_directory: directory.join("data"),
        web_directory: directory.join("web"),
        tenant_id: "local".into(),
        users: vec![User {
            principal: Principal {
                id: "admin".into(),
                tenant_id: "local".into(),
                role: Role::Admin,
                store_ids: vec![],
            },
            token_sha256: token_hash(&token),
        }],
        apic: None,
    };
    fn private_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
        use std::io::Write;
        let mut o = fs::OpenOptions::new();
        o.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            o.mode(0o600);
        }
        let mut f = o
            .open(path)
            .map_err(|_| "file exists or cannot be created; initialization never overwrites")?;
        f.write_all(bytes)
            .and_then(|_| f.sync_all())
            .map_err(|_| "cannot persist initialization file".into())
    }
    if directory.join("config.json").exists() || directory.join("initial-admin-token.txt").exists()
    {
        return Err("installation already initialized".into());
    }
    private_write(&directory.join("initial-admin-token.txt"), token.as_bytes())?;
    private_write(
        &directory.join("config.json"),
        &serde_json::to_vec_pretty(&c).map_err(|_| "cannot encode config")?,
    )?;
    Ok(())
}
