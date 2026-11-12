use common::error::CommonError;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub struct ReleaseAcquisition;

impl ReleaseAcquisition {
    /// Safe zip extraction: extracts exactly `target_filename` from `zip_bytes` into `dest_dir`,
    /// enforcing directory traversal guards and size limits.
    pub fn extract_member_safe(
        zip_bytes: &[u8],
        target_filename: &str,
        dest_dir: &Path,
        max_size_bytes: u64,
    ) -> Result<PathBuf, CommonError> {
        let cursor = std::io::Cursor::new(zip_bytes);
        let mut archive = zip::ZipArchive::new(cursor)
            .map_err(|e| CommonError::Validation(format!("Invalid zip archive: {}", e)))?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)
                .map_err(|e| CommonError::Validation(format!("Zip index error: {}", e)))?;

            let enclosed_name = file.enclosed_name().ok_or_else(|| {
                CommonError::Security(format!("Illegal path in zip: {}", file.name()))
            })?;

            if enclosed_name.file_name().and_then(|f| f.to_str()) == Some(target_filename) {
                if file.size() > max_size_bytes {
                    return Err(CommonError::Security(format!(
                        "Zip member {} exceeds size limit ({} > {})",
                        target_filename,
                        file.size(),
                        max_size_bytes
                    )));
                }

                let outpath = dest_dir.join(target_filename);
                let mut outfile = File::create(&outpath)?;
                let mut buffer = Vec::new();
                file.read_to_end(&mut buffer)?;
                outfile.write_all(&buffer)?;
                return Ok(outpath);
            }
        }

        Err(CommonError::Validation(format!(
            "Target file '{}' not found in zip archive",
            target_filename
        )))
    }

    /// Download release asset from GitHub or local mock
    pub async fn fetch_asset_bytes(url: &str) -> Result<Vec<u8>, CommonError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| CommonError::Generic(format!("HTTP client error: {}", e)))?;

        let resp = client
            .get(url)
            .header("User-Agent", "AGY-Patcher-Rust/0.1.0")
            .header("Accept", "application/octet-stream")
            .send()
            .await
            .map_err(|e| CommonError::Generic(format!("Fetch failed: {}", e)))?;

        if !resp.status().is_success() {
            return Err(CommonError::Generic(format!(
                "HTTP error: status {}",
                resp.status()
            )));
        }

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| CommonError::Generic(format!("Read bytes error: {}", e)))?;

        Ok(bytes.to_vec())
    }
}
