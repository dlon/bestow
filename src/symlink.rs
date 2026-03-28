use std::path::{Path, PathBuf};

use crate::StowError;

/// Create a symlink at `dst` pointing to `src`.
pub fn create_symlink(src: &Path, dst: &Path) -> Result<(), StowError> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(src, dst).map_err(|e| StowError::Io {
            path: dst.to_path_buf(),
            source: e,
        })
    }
    #[cfg(windows)]
    {
        // On Windows, we need to distinguish file vs dir symlinks.
        // Requires Developer Mode or elevated privileges.
        if src.is_dir() {
            std::os::windows::fs::symlink_dir(src, dst).map_err(|e| StowError::Io {
                path: dst.to_path_buf(),
                source: e,
            })
        } else {
            std::os::windows::fs::symlink_file(src, dst).map_err(|e| StowError::Io {
                path: dst.to_path_buf(),
                source: e,
            })
        }
    }
}

/// Read the target of a symlink.
pub fn read_link_target(path: &Path) -> Result<PathBuf, StowError> {
    std::fs::read_link(path).map_err(|e| StowError::Io {
        path: path.to_path_buf(),
        source: e,
    })
}

/// Returns true if `path` is a symlink (does not follow the link).
pub fn is_symlink(path: &Path) -> bool {
    path.symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

/// Returns true if `path` is a symlink whose target is inside `stow_dir`.
pub fn is_stow_symlink(path: &Path, stow_dir: &Path) -> bool {
    if !is_symlink(path) {
        return false;
    }
    let Ok(target) = read_link_target(path) else {
        return false;
    };
    // Resolve relative symlinks relative to the symlink's parent dir.
    let resolved = if target.is_absolute() {
        target
    } else {
        let parent = path.parent().unwrap_or(Path::new("."));
        parent.join(&target)
    };
    // Canonicalize to resolve `..` components.
    let resolved = std::fs::canonicalize(&resolved).unwrap_or(resolved);
    let stow_dir = std::fs::canonicalize(stow_dir).unwrap_or(stow_dir.to_path_buf());
    resolved.starts_with(&stow_dir)
}
