use common::error::CommonError;
use common::fs::ensure_within_root;
use std::path::{Path, PathBuf};

pub struct Sandbox {
    pub root: PathBuf,
}

impl Sandbox {
    pub fn new<P: AsRef<Path>>(root: P) -> Self {
        Sandbox {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn resolve_safe_path<P: AsRef<Path>>(&self, relative_or_absolute: P) -> Result<PathBuf, CommonError> {
        ensure_within_root(&self.root, relative_or_absolute.as_ref())
    }

    pub fn redact_secrets(input: &str) -> String {
        let mut text = input.to_string();
        let sensitive_keys = ["sk-", "ghp_", "bearer ", "token=", "key="];
        for marker in sensitive_keys {
            if let Some(pos) = text.to_lowercase().find(marker) {
                let end = text[pos..].find(|c: char| c.is_whitespace() || c == '"' || c == '\'').map(|idx| pos + idx).unwrap_or(text.len());
                if end > pos + marker.len() {
                    let secret_len = end - (pos + marker.len());
                    if secret_len > 6 {
                        text.replace_range((pos + marker.len())..end, "[REDACTED]");
                    }
                }
            }
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_redaction() {
        let sample = "Authorization: Bearer sk-1234567890abcdef1234567890abcdef\n";
        let redacted = Sandbox::redact_secrets(sample);
        assert!(!redacted.contains("sk-1234567890abcdef"));
        assert!(redacted.contains("[REDACTED]"));
    }
}
