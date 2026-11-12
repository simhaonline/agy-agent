use common::error::CommonError;
use common::hash::{compute_sha256_bytes, compute_sha256_file};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct TransactionalApplier;

impl TransactionalApplier {
    pub fn apply<P: AsRef<Path>>(
        target_output: P,
        patched_bytes: &[u8],
        expected_hash: &str,
    ) -> Result<PathBuf, CommonError> {
        let target_path = target_output.as_ref();
        let target_dir = target_path
            .parent()
            .unwrap_or_else(|| Path::new("."));

        if !target_dir.exists() {
            fs::create_dir_all(target_dir)?;
        }

        // 1. Stage in temporary file beside target
        let staging_name = format!(
            ".agy_stage_{}",
            uuid::Uuid::new_v4().to_string().replace('-', "")
        );
        let staging_path = target_dir.join(staging_name);

        {
            let mut file = File::create(&staging_path)?;
            file.write_all(patched_bytes)?;
            file.flush()?;
            file.sync_all()?;
        }

        // 2. Verify staged file
        let staged_hash = compute_sha256_file(&staging_path)?;
        let in_memory_hash = compute_sha256_bytes(patched_bytes);

        if staged_hash != in_memory_hash || staged_hash != expected_hash {
            let _ = fs::remove_file(&staging_path);
            return Err(CommonError::Validation(format!(
                "Staged file hash mismatch: expected {}, got {}",
                expected_hash, staged_hash
            )));
        }

        // 3. Atomically replace or rename
        if target_path.exists() {
            // On Windows, direct rename over existing might fail if file locked, but if not locked, remove then rename
            fs::remove_file(target_path)?;
        }
        fs::rename(&staging_path, target_path)?;

        // 4. Final verification
        let final_hash = compute_sha256_file(target_path)?;
        if final_hash != expected_hash {
            return Err(CommonError::Validation(format!(
                "Committed file hash mismatch: expected {}, got {}",
                expected_hash, final_hash
            )));
        }

        Ok(target_path.to_path_buf())
    }
}
