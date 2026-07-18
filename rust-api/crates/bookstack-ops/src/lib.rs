//! Operational features: BookStack-instance import (rate-limit aware),
//! encrypted backups to object storage / filesystem, and realtime WAL
//! shipping. See each module for details.

pub mod backup;
pub mod crypto;
pub mod import;
pub mod storage;
pub mod walship;

pub use storage::Target;
pub use walship::{WalShipConfig, WalShipper};

#[derive(Debug, thiserror::Error)]
pub enum OpsError {
    #[error("{0}")]
    Config(String),
    #[error("crypto: {0}")]
    Crypto(String),
    #[error("storage: {0}")]
    Storage(String),
    #[error("import: {0}")]
    Import(String),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    Core(#[from] bookstack_core::CoreError),
}

pub type Result<T> = std::result::Result<T, OpsError>;
