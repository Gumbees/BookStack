//! Realtime SQL (WAL) shipping: a managed `pg_receivewal` replication
//! stream writes WAL segments into a spool directory; completed segments are
//! encrypted and shipped to object storage or the filesystem target as they
//! close, then removed from the spool. This is continuous, near-realtime
//! physical replication of every database change — pair it with periodic
//! `sql` (pg_dump) backups as restore baselines.
//!
//! Requirements: `pg_receivewal` on PATH and a DATABASE_URL role with the
//! REPLICATION attribute (plus pg_hba allowing replication connections).

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use crate::storage::{Storage, Target};
use crate::{backup, crypto, OpsError, Result};

pub struct WalShipConfig {
    pub database_url: String,
    pub target: Target,
    pub spool_dir: PathBuf,
    pub slot: String,
}

impl WalShipConfig {
    /// Enabled when WALSHIP_ENABLED=true. Target from WALSHIP_TARGET
    /// (object_storage | filesystem, default filesystem).
    pub fn from_env(database_url: &str) -> Option<WalShipConfig> {
        if std::env::var("WALSHIP_ENABLED").ok().as_deref() != Some("true") {
            return None;
        }
        let target = Target::parse(
            &std::env::var("WALSHIP_TARGET").unwrap_or_else(|_| "filesystem".into()),
        )
        .ok()?;
        Some(WalShipConfig {
            database_url: database_url.to_string(),
            target,
            spool_dir: PathBuf::from(
                std::env::var("WALSHIP_SPOOL_DIR").unwrap_or_else(|_| "var/walspool".into()),
            ),
            slot: std::env::var("WALSHIP_SLOT").unwrap_or_else(|_| "bookstack_walship".into()),
        })
    }
}

pub struct WalShipper {
    config: WalShipConfig,
    pub running: AtomicBool,
    pub shipped_segments: AtomicI64,
    pub last_segment: Mutex<Option<String>>,
    pub last_error: Mutex<Option<String>>,
}

impl WalShipper {
    pub fn start(config: WalShipConfig) -> Result<Arc<WalShipper>> {
        backup::passphrase()?; // fail fast: shipping is always encrypted
        std::fs::create_dir_all(&config.spool_dir)
            .map_err(|e| OpsError::Config(format!("WALSHIP_SPOOL_DIR: {e}")))?;
        Storage::for_target(config.target)?; // fail fast on target misconfig
        let shipper = Arc::new(WalShipper {
            config,
            running: AtomicBool::new(false),
            shipped_segments: AtomicI64::new(0),
            last_segment: Mutex::new(None),
            last_error: Mutex::new(None),
        });
        tokio::spawn(receiver_loop(shipper.clone()));
        tokio::spawn(shipper_loop(shipper.clone()));
        Ok(shipper)
    }

    pub fn status(&self) -> Value {
        json!({
            "enabled": true,
            "running": self.running.load(Ordering::SeqCst),
            "target": self.config.target.as_str(),
            "slot": self.config.slot,
            "spool_dir": self.config.spool_dir.display().to_string(),
            "shipped_segments": self.shipped_segments.load(Ordering::SeqCst),
            "last_segment": self.last_segment.lock().unwrap().clone(),
            "last_error": self.last_error.lock().unwrap().clone(),
        })
    }

    fn record_error(&self, err: impl std::fmt::Display) {
        *self.last_error.lock().unwrap() = Some(err.to_string());
    }
}

/// Keep a pg_receivewal child running (restart with backoff on exit).
/// `--create-slot` is a standalone action mode (it creates the slot and
/// exits), so the slot is ensured first and streaming runs separately.
async fn receiver_loop(shipper: Arc<WalShipper>) {
    let mut slot_ensured = false;
    loop {
        if !slot_ensured {
            let created = tokio::process::Command::new("pg_receivewal")
                .arg("--create-slot")
                .arg("--if-not-exists")
                .arg("--slot")
                .arg(&shipper.config.slot)
                .arg("--dbname")
                .arg(&shipper.config.database_url)
                .output()
                .await;
            match created {
                Ok(output) if output.status.success() => slot_ensured = true,
                Ok(output) => {
                    shipper.record_error(format!(
                        "slot creation failed: {}",
                        String::from_utf8_lossy(&output.stderr).trim()
                    ));
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    continue;
                }
                Err(err) => {
                    shipper.record_error(format!("pg_receivewal not runnable: {err}"));
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    continue;
                }
            }
        }

        let spawn = tokio::process::Command::new("pg_receivewal")
            .arg("--directory")
            .arg(&shipper.config.spool_dir)
            .arg("--slot")
            .arg(&shipper.config.slot)
            .arg("--no-loop")
            .arg("--dbname")
            .arg(&shipper.config.database_url)
            .kill_on_drop(true)
            .stderr(std::process::Stdio::piped())
            .spawn();

        match spawn {
            Ok(mut child) => {
                shipper.running.store(true, Ordering::SeqCst);
                tracing::info!("pg_receivewal streaming into {}", shipper.config.spool_dir.display());
                let stderr = child.stderr.take();
                let status = child.wait().await;
                shipper.running.store(false, Ordering::SeqCst);
                let mut detail = String::new();
                if let Some(mut err_pipe) = stderr {
                    use tokio::io::AsyncReadExt;
                    let _ = err_pipe.read_to_string(&mut detail).await;
                }
                shipper.record_error(format!(
                    "pg_receivewal exited ({:?}): {}",
                    status.map(|s| s.code()).unwrap_or(None),
                    detail.trim().chars().take(400).collect::<String>()
                ));
            }
            Err(err) => {
                shipper.running.store(false, Ordering::SeqCst);
                shipper.record_error(format!("pg_receivewal not runnable: {err}"));
            }
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

/// Watch the spool; encrypt + upload completed segments (no `.partial`).
async fn shipper_loop(shipper: Arc<WalShipper>) {
    let key = match backup::passphrase() {
        Ok(key) => key,
        Err(err) => {
            shipper.record_error(err);
            return;
        }
    };
    let mut in_flight: HashSet<String> = HashSet::new();
    loop {
        tokio::time::sleep(Duration::from_secs(3)).await;
        let entries = match std::fs::read_dir(&shipper.config.spool_dir) {
            Ok(entries) => entries,
            Err(err) => {
                shipper.record_error(err);
                continue;
            }
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            // Completed WAL segments are 24-hex-char names (plus .history
            // files); anything ending in .partial is still being written.
            if name.ends_with(".partial") || in_flight.contains(&name) {
                continue;
            }
            in_flight.insert(name.clone());
            let path = entry.path();
            let result: Result<()> = async {
                let raw = tokio::fs::read(&path).await.map_err(|e| OpsError::Storage(e.to_string()))?;
                let sealed = crypto::encrypt(&key, &crypto::gzip(&raw)?)?;
                let storage = Storage::for_target(shipper.config.target)?;
                storage.put(&format!("wal/{name}.gz.enc"), sealed).await?;
                tokio::fs::remove_file(&path).await.map_err(|e| OpsError::Storage(e.to_string()))?;
                Ok(())
            }
            .await;
            match result {
                Ok(()) => {
                    shipper.shipped_segments.fetch_add(1, Ordering::SeqCst);
                    *shipper.last_segment.lock().unwrap() = Some(name.clone());
                    tracing::info!("shipped WAL segment {name}");
                    in_flight.remove(&name);
                }
                Err(err) => {
                    tracing::warn!("WAL ship failed for {name}: {err}");
                    shipper.record_error(format!("{name}: {err}"));
                    in_flight.remove(&name);
                }
            }
        }
    }
}
