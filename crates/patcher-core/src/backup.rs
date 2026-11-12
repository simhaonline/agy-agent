use common::error::CommonError;
use common::hash::compute_sha256_file;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupRecord {
    pub source_path: PathBuf,
    pub backup_path: PathBuf,
    pub original_sha256: String,
    pub size_bytes: u64,
    pub created_at: String,
}

pub struct BackupManager;

impl BackupManager {
    pub fn get_default_backup_path<P: AsRef<Path>>(target: P) -> PathBuf {
        let mut path = target.as_ref().to_path_buf();
        let orig_ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if orig_ext.is_empty() {
            path.set_extension("original");
        } else {
            path.set_extension(format!("{}.original", orig_ext));
        }
        path
    }

    pub fn create_verified_backup<P: AsRef<Path>>(
        target: P,
        explicit_backup: Option<P>,
    ) -> Result<BackupRecord, CommonError> {
        let target_path = target.as_ref();
        if !target_path.exists() {
            return Err(CommonError::Validation(format!(
                "Source binary {:?} does not exist",
                target_path
            )));
        }

        let source_hash = compute_sha256_file(target_path)?;
        let metadata = fs::metadata(target_path)?;
        let size_bytes = metadata.len();

        let backup_path = if let Some(ref bp) = explicit_backup {
            bp.as_ref().to_path_buf()
        } else {
            Self::get_default_backup_path(target_path)
        };

        if backup_path.exists() {
            // Verify if existing backup matches current target
            let existing_hash = compute_sha256_file(&backup_path)?;
            if existing_hash == source_hash {
                return Ok(BackupRecord {
                    source_path: target_path.to_path_buf(),
                    backup_path,
                    original_sha256: source_hash,
                    size_bytes,
                    created_at: chrono::Utc::now().to_rfc3339(),
                });
            } else {
                return Err(CommonError::Validation(format!(
                    "Existing backup {:?} does not match source binary hash! Stale backup reuse prevented.",
                    backup_path
                )));
            }
        }

        fs::copy(target_path, &backup_path)?;

        let backup_hash = compute_sha256_file(&backup_path)?;
        if backup_hash != source_hash {
            let _ = fs::remove_file(&backup_path);
            return Err(CommonError::Validation(
                "Backup creation failed hash verification check".into(),
            ));
        }

        Ok(BackupRecord {
            source_path: target_path.to_path_buf(),
            backup_path,
            original_sha256: source_hash,
            size_bytes,
            created_at: chrono::Utc::now().to_rfc3339(),
        })
    }
}
