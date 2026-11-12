use common::error::CommonError;
use common::hash::compute_sha256_file;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VerificationResult {
    pub is_valid: bool,
    pub expected_hash: String,
    pub actual_hash: String,
    pub size_bytes: u64,
    pub message: String,
}

pub struct Verifier;

impl Verifier {
    pub fn verify_file<P: AsRef<Path>>(
        file_path: P,
        expected_hash: &str,
        expected_size: Option<u64>,
    ) -> Result<VerificationResult, CommonError> {
        let p = file_path.as_ref();
        if !p.exists() {
            return Err(CommonError::Validation(format!("File {:?} does not exist", p)));
        }

        let actual_hash = compute_sha256_file(p)?;
        let metadata = fs::metadata(p)?;
        let size_bytes = metadata.len();

        let mut is_valid = actual_hash.eq_ignore_ascii_case(expected_hash);
        let mut message = if is_valid {
            "SHA-256 match verified".to_string()
        } else {
            format!(
                "SHA-256 mismatch: expected {}, got {}",
                expected_hash, actual_hash
            )
        };

        if let Some(exp_sz) = expected_size {
            if size_bytes != exp_sz {
                is_valid = false;
                message.push_str(&format!(
                    "; Size mismatch: expected {} bytes, got {} bytes",
                    exp_sz, size_bytes
                ));
            }
        }

        Ok(VerificationResult {
            is_valid,
            expected_hash: expected_hash.to_string(),
            actual_hash,
            size_bytes,
            message,
        })
    }
}
