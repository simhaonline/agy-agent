use common::error::CommonError;
use common::hash::compute_sha256_file;
use std::fs;
use std::path::{Path, PathBuf};

pub struct RestoreManager;

impl RestoreManager {
    pub fn restore<P: AsRef<Path>>(target: P, explicit_backup: Option<P>) -> Result<PathBuf, CommonError> {
        let target_path = target.as_ref();
        let backup_path = if let Some(ref bp) = explicit_backup {
            bp.as_ref().to_path_buf()
        } else {
            crate::backup::BackupManager::get_default_backup_path(target_path)
        };

        if !backup_path.exists() {
            return Err(CommonError::Validation(format!(
                "Backup {:?} not found",
                backup_path
            )));
        }

        let backup_hash = compute_sha256_file(&backup_path)?;

        if target_path.exists() {
            fs::remove_file(target_path)?;
        }

        fs::copy(&backup_path, target_path)?;

        let restored_hash = compute_sha256_file(target_path)?;
        if restored_hash != backup_hash {
            return Err(CommonError::Validation(
                "Restored file failed hash verification against backup".into(),
            ));
        }

        Ok(target_path.to_path_buf())
    }
}
