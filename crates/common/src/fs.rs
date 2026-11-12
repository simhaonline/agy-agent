use crate::error::CommonError;
use std::path::{Component, Path, PathBuf};

/// Check if a target path is safely contained inside the root directory.
/// Prevents directory traversal attacks like `../../windows/system32`.
pub fn ensure_within_root(root: &Path, target: &Path) -> Result<PathBuf, CommonError> {
    let canonical_root = root
        .canonicalize()
        .map_err(|e| CommonError::Io(e))?;

    // Check if target is relative to root
    let combined = if target.is_absolute() {
        target.to_path_buf()
    } else {
        root.join(target)
    };

    // Normalize components
    let mut normalized = PathBuf::new();
    for comp in combined.components() {
        match comp {
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(c) => {
                normalized.push(c);
            }
            Component::RootDir => {
                normalized.push(comp.as_os_str());
            }
            Component::Prefix(p) => {
                normalized.push(p.as_os_str());
            }
            Component::CurDir => {}
        }
    }

    // If file exists, check canonicalized
    if normalized.exists() {
        let canonical_target = normalized.canonicalize().map_err(CommonError::Io)?;
        if !canonical_target.starts_with(&canonical_root) {
            return Err(CommonError::Security(format!(
                "Path {:?} escapes root directory {:?}",
                target, root
            )));
        }
        Ok(canonical_target)
    } else {
        // If file doesn't exist yet, check its parent or prefix against root
        if !normalized.starts_with(&canonical_root) && !normalized.starts_with(root) {
            return Err(CommonError::Security(format!(
                "Path {:?} would escape root directory {:?}",
                target, root
            )));
        }
        Ok(normalized)
    }
}
