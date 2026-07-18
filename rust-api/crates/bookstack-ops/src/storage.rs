//! Backup targets behind one interface: S3-compatible object storage
//! (BACKUP_S3_* env) and the local filesystem (global backups only —
//! enforced by the API layer).

use std::sync::Arc;

use object_store::aws::AmazonS3Builder;
use object_store::local::LocalFileSystem;
use object_store::path::Path as StorePath;
use object_store::ObjectStore;

use crate::{OpsError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    ObjectStorage,
    Filesystem,
}

impl Target {
    pub fn parse(s: &str) -> Result<Target> {
        match s {
            "object_storage" | "s3" => Ok(Target::ObjectStorage),
            "filesystem" | "fs" => Ok(Target::Filesystem),
            other => Err(OpsError::Config(format!(
                "unknown target '{other}' (expected object_storage or filesystem)"
            ))),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Target::ObjectStorage => "object_storage",
            Target::Filesystem => "filesystem",
        }
    }
}

pub struct Storage {
    pub target: Target,
    store: Arc<dyn ObjectStore>,
    prefix: String,
    /// Human-readable location base recorded in backup metadata.
    pub location_base: String,
}

impl Storage {
    /// Object storage from BACKUP_S3_* env vars.
    pub fn object_storage() -> Result<Storage> {
        let bucket = std::env::var("BACKUP_S3_BUCKET")
            .map_err(|_| OpsError::Config("object storage is not configured (set BACKUP_S3_BUCKET, BACKUP_S3_ACCESS_KEY_ID, BACKUP_S3_SECRET_ACCESS_KEY; optional BACKUP_S3_ENDPOINT/REGION)".into()))?;
        let mut builder = AmazonS3Builder::new()
            .with_bucket_name(&bucket)
            .with_region(std::env::var("BACKUP_S3_REGION").unwrap_or_else(|_| "us-east-1".into()));
        if let Ok(endpoint) = std::env::var("BACKUP_S3_ENDPOINT") {
            builder = builder.with_endpoint(&endpoint).with_virtual_hosted_style_request(false);
            if endpoint.starts_with("http://") {
                builder = builder.with_allow_http(true);
            }
        }
        if let Ok(key) = std::env::var("BACKUP_S3_ACCESS_KEY_ID") {
            builder = builder.with_access_key_id(key);
        }
        if let Ok(secret) = std::env::var("BACKUP_S3_SECRET_ACCESS_KEY") {
            builder = builder.with_secret_access_key(secret);
        }
        let store = builder.build().map_err(|e| OpsError::Config(e.to_string()))?;
        let prefix = std::env::var("BACKUP_S3_PREFIX").unwrap_or_else(|_| "bookstack".into());
        Ok(Storage {
            target: Target::ObjectStorage,
            store: Arc::new(store),
            location_base: format!("s3://{bucket}/{prefix}"),
            prefix,
        })
    }

    /// Filesystem target rooted at BACKUP_DIR (created if missing).
    pub fn filesystem() -> Result<Storage> {
        let dir = std::env::var("BACKUP_DIR").unwrap_or_else(|_| "var/backups".into());
        std::fs::create_dir_all(&dir).map_err(|e| OpsError::Config(format!("BACKUP_DIR: {e}")))?;
        let canonical = std::fs::canonicalize(&dir).map_err(|e| OpsError::Config(e.to_string()))?;
        let store = LocalFileSystem::new_with_prefix(&canonical)
            .map_err(|e| OpsError::Config(e.to_string()))?;
        Ok(Storage {
            target: Target::Filesystem,
            store: Arc::new(store),
            prefix: String::new(),
            location_base: canonical.display().to_string(),
        })
    }

    pub fn for_target(target: Target) -> Result<Storage> {
        match target {
            Target::ObjectStorage => Storage::object_storage(),
            Target::Filesystem => Storage::filesystem(),
        }
    }

    fn path(&self, name: &str) -> StorePath {
        if self.prefix.is_empty() {
            StorePath::from(name)
        } else {
            StorePath::from(format!("{}/{}", self.prefix, name))
        }
    }

    pub async fn put(&self, name: &str, data: Vec<u8>) -> Result<String> {
        let path = self.path(name);
        self.store
            .put(&path, data.into())
            .await
            .map_err(|e| OpsError::Storage(e.to_string()))?;
        Ok(format!("{}/{}", self.location_base, name))
    }

    pub async fn get(&self, name: &str) -> Result<Vec<u8>> {
        let path = self.path(name);
        let result = self
            .store
            .get(&path)
            .await
            .map_err(|e| OpsError::Storage(e.to_string()))?;
        let bytes = result.bytes().await.map_err(|e| OpsError::Storage(e.to_string()))?;
        Ok(bytes.to_vec())
    }
}
