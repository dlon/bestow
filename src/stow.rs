use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::{
    Config, StowError,
    conflict::{ConflictKind, ConflictSet},
    ignore::Patterns,
    symlink::{create_symlink, is_stow_symlink, is_symlink, read_link_target},
};

/// A planned filesystem action collected before any changes are made.
#[derive(Debug)]
pub enum Action {
    CreateSymlink {
        src: PathBuf,
        dst: PathBuf,
    },
    CreateDir {
        path: PathBuf,
    },
    RemoveSymlink {
        path: PathBuf,
    },
    /// Unfold a folded directory symlink into a real dir with per-file symlinks.
    Unfold {
        dir: PathBuf,
        existing_link_target: PathBuf,
    },
}

impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Action::CreateSymlink { src, dst } => {
                write!(f, "symlink {} -> {}", dst.display(), src.display())
            }
            Action::CreateDir { path } => write!(f, "mkdir {}", path.display()),
            Action::RemoveSymlink { path } => write!(f, "remove symlink {}", path.display()),
            Action::Unfold {
                dir,
                existing_link_target,
            } => write!(
                f,
                "unfold {} (was -> {})",
                dir.display(),
                existing_link_target.display()
            ),
        }
    }
}

/// Compute a relative path from `from` (a file path) to `to`.
/// Both paths must be absolute.
fn relative_path(from: &Path, to: &Path) -> PathBuf {
    // from is the symlink location; we need the path relative to its parent dir.
    let from_dir = from.parent().unwrap_or(Path::new("/"));
    let mut from_components: Vec<_> = from_dir.components().collect();
    let mut to_components: Vec<_> = to.components().collect();

    // Strip common prefix
    let common_len = from_components
        .iter()
        .zip(to_components.iter())
        .take_while(|(a, b)| a == b)
        .count();
    from_components.drain(..common_len);
    to_components.drain(..common_len);

    let mut rel = PathBuf::new();
    for _ in &from_components {
        rel.push("..");
    }
    for c in &to_components {
        rel.push(c);
    }
    if rel.as_os_str().is_empty() {
        rel.push(".");
    }
    rel
}

/// Collect stow actions for a single package.
pub fn plan_stow(
    package: &str,
    config: &Config,
    patterns: &Patterns,
    conflicts: &mut ConflictSet,
) -> Result<Vec<Action>, StowError> {
    let package_dir = config.stow_dir.join(package);
    if !package_dir.is_dir() {
        return Err(StowError::PackageNotFound(package.to_string()));
    }
    // Canonicalize so relative symlink targets are computed correctly on
    // platforms where temp/home dirs involve symlinks (e.g. /var → /private/var on macOS).
    let package_dir = std::fs::canonicalize(&package_dir).unwrap_or(package_dir);
    let target_dir = std::fs::canonicalize(&config.target_dir).unwrap_or(config.target_dir.clone());

    let mut actions = Vec::new();
    plan_stow_dir(
        &package_dir,
        &target_dir,
        &package_dir,
        config,
        patterns,
        conflicts,
        &mut actions,
    )?;
    Ok(actions)
}

fn plan_stow_dir(
    src_dir: &Path,
    dst_dir: &Path,
    package_root: &Path,
    config: &Config,
    patterns: &Patterns,
    conflicts: &mut ConflictSet,
    actions: &mut Vec<Action>,
) -> Result<(), StowError> {
    let entries = std::fs::read_dir(src_dir).map_err(|e| StowError::Io {
        path: src_dir.to_path_buf(),
        source: e,
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| StowError::Io {
            path: src_dir.to_path_buf(),
            source: e,
        })?;
        let src_path = entry.path();

        if patterns.should_ignore(&src_path) {
            continue;
        }

        let file_name = entry.file_name();
        let dst_path = dst_dir.join(&file_name);

        let src_is_dir = src_path.is_dir() && !is_symlink(&src_path);

        if src_is_dir {
            // Try to fold: can we represent the entire subtree as a single dir symlink?
            if !dst_path.exists() && !is_symlink(&dst_path) {
                // Target doesn't exist at all — fold the whole directory
                let src_abs = std::fs::canonicalize(&src_path).unwrap_or_else(|_| src_path.clone());
                let rel = relative_path(&dst_path, &src_abs);
                actions.push(Action::CreateSymlink {
                    src: rel,
                    dst: dst_path,
                });
            } else if is_symlink(&dst_path) && is_stow_symlink(&dst_path, &config.stow_dir) {
                // It's a folded dir from stow — need to unfold before descending
                let link_target = read_link_target(&dst_path)?;
                let abs_target = if link_target.is_absolute() {
                    link_target.clone()
                } else {
                    let parent = dst_path.parent().unwrap_or(Path::new("."));
                    parent.join(&link_target)
                };

                // Check if symlink already points to our source directory (already stowed)
                let src_canon =
                    std::fs::canonicalize(&src_path).unwrap_or_else(|_| src_path.clone());
                let dst_canon =
                    std::fs::canonicalize(&abs_target).unwrap_or_else(|_| abs_target.clone());
                if src_canon == dst_canon {
                    // Already correctly stowed — nothing to do
                } else if abs_target.is_dir() {
                    // Different stow package owns this dir — unfold and merge
                    actions.push(Action::Unfold {
                        dir: dst_path.clone(),
                        existing_link_target: link_target,
                    });
                    // After unfolding, we recurse as if dst_path is a real dir.
                    // We plan into a "virtual" real dir state.
                    plan_stow_dir_after_unfold(
                        &src_path,
                        &dst_path,
                        package_root,
                        config,
                        patterns,
                        conflicts,
                        actions,
                    )?;
                } else {
                    conflicts.add(dst_path, ConflictKind::ExistingFile);
                }
            } else if dst_path.is_dir() && !is_symlink(&dst_path) {
                // Real dir exists — recurse
                plan_stow_dir(
                    &src_path,
                    &dst_path,
                    package_root,
                    config,
                    patterns,
                    conflicts,
                    actions,
                )?;
            } else {
                // Something else is in the way
                if !patterns.should_defer(&dst_path) && !patterns.should_override(&dst_path) {
                    conflicts.add(dst_path, ConflictKind::ExistingFile);
                }
            }
        } else {
            // src is a file (or a symlink in the package)
            if !dst_path.exists() && !is_symlink(&dst_path) {
                let src_abs = std::fs::canonicalize(&src_path).unwrap_or_else(|_| src_path.clone());
                let rel = relative_path(&dst_path, &src_abs);
                actions.push(Action::CreateSymlink {
                    src: rel,
                    dst: dst_path,
                });
            } else if is_symlink(&dst_path) {
                if is_stow_symlink(&dst_path, &config.stow_dir) {
                    // Check if it points to our same package
                    let link_target = read_link_target(&dst_path)?;
                    let abs_target = if link_target.is_absolute() {
                        link_target
                    } else {
                        let parent = dst_path.parent().unwrap_or(Path::new("."));
                        parent.join(link_target)
                    };
                    let pkg_abs =
                        std::fs::canonicalize(&src_path).unwrap_or_else(|_| src_path.clone());
                    let tgt_abs = std::fs::canonicalize(&abs_target).unwrap_or(abs_target);
                    if pkg_abs == tgt_abs {
                        // Already stowed from same package, skip
                    } else if patterns.should_defer(&dst_path) {
                        // defer: skip
                    } else if patterns.should_override(&dst_path) {
                        // override: replace
                        actions.push(Action::RemoveSymlink {
                            path: dst_path.clone(),
                        });
                        let rel = relative_path(&dst_path, &pkg_abs);
                        actions.push(Action::CreateSymlink {
                            src: rel,
                            dst: dst_path,
                        });
                    } else {
                        conflicts.add(
                            dst_path.clone(),
                            ConflictKind::OwnedByOther { owner: tgt_abs },
                        );
                    }
                } else {
                    // Symlink not owned by stow
                    if config.adopt {
                        // Move existing file into package, then stow
                        let src_abs =
                            std::fs::canonicalize(&src_path).unwrap_or_else(|_| src_path.clone());
                        let rel = relative_path(&dst_path, &src_abs);
                        actions.push(Action::RemoveSymlink {
                            path: dst_path.clone(),
                        });
                        actions.push(Action::CreateSymlink {
                            src: rel,
                            dst: dst_path,
                        });
                    } else if patterns.should_defer(&dst_path) {
                        // defer
                    } else if patterns.should_override(&dst_path) {
                        let src_abs =
                            std::fs::canonicalize(&src_path).unwrap_or_else(|_| src_path.clone());
                        let rel = relative_path(&dst_path, &src_abs);
                        actions.push(Action::RemoveSymlink {
                            path: dst_path.clone(),
                        });
                        actions.push(Action::CreateSymlink {
                            src: rel,
                            dst: dst_path,
                        });
                    } else {
                        conflicts.add(dst_path, ConflictKind::ExistingFile);
                    }
                }
            } else if dst_path.exists() {
                // Regular file in the way
                if patterns.should_defer(&dst_path) {
                    // defer: skip
                } else if config.adopt {
                    // Move it into the package first
                    let src_abs =
                        std::fs::canonicalize(&src_path).unwrap_or_else(|_| src_path.clone());
                    let rel = relative_path(&dst_path, &src_abs);
                    // We push a special "adopt" version — handled in execute
                    actions.push(Action::RemoveSymlink {
                        path: dst_path.clone(),
                    });
                    actions.push(Action::CreateSymlink {
                        src: rel,
                        dst: dst_path,
                    });
                } else if patterns.should_override(&dst_path) {
                    let src_abs =
                        std::fs::canonicalize(&src_path).unwrap_or_else(|_| src_path.clone());
                    let rel = relative_path(&dst_path, &src_abs);
                    actions.push(Action::RemoveSymlink {
                        path: dst_path.clone(),
                    });
                    actions.push(Action::CreateSymlink {
                        src: rel,
                        dst: dst_path,
                    });
                } else {
                    conflicts.add(dst_path, ConflictKind::ExistingFile);
                }
            }
        }
    }
    Ok(())
}

/// Plan stow into a directory that will be unfolded at execution time.
/// The `Action::Unfold` already handles re-creating the old package's symlinks,
/// so here we only need to plan the new package's files.
fn plan_stow_dir_after_unfold(
    src_dir: &Path,
    dst_dir: &Path,
    package_root: &Path,
    config: &Config,
    patterns: &Patterns,
    conflicts: &mut ConflictSet,
    actions: &mut Vec<Action>,
) -> Result<(), StowError> {
    plan_stow_dir(
        src_dir,
        dst_dir,
        package_root,
        config,
        patterns,
        conflicts,
        actions,
    )
}

/// Collect unstow actions for a single package.
pub fn plan_unstow(
    package: &str,
    config: &Config,
    patterns: &Patterns,
) -> Result<Vec<Action>, StowError> {
    let package_dir = config.stow_dir.join(package);
    if !package_dir.is_dir() {
        return Err(StowError::PackageNotFound(package.to_string()));
    }
    // Canonicalize so symlink resolution works correctly on platforms where
    // temp/home dirs involve symlinks (e.g. /var → /private/var on macOS).
    let package_dir = std::fs::canonicalize(&package_dir).unwrap_or(package_dir);
    let target_dir = std::fs::canonicalize(&config.target_dir).unwrap_or(config.target_dir.clone());
    let mut actions = Vec::new();
    plan_unstow_dir(&package_dir, &target_dir, config, patterns, &mut actions)?;
    Ok(actions)
}

fn plan_unstow_dir(
    src_dir: &Path,
    dst_dir: &Path,
    config: &Config,
    patterns: &Patterns,
    actions: &mut Vec<Action>,
) -> Result<(), StowError> {
    let Ok(read) = std::fs::read_dir(src_dir) else {
        return Ok(());
    };

    for entry in read {
        let entry = entry.map_err(|e| StowError::Io {
            path: src_dir.to_path_buf(),
            source: e,
        })?;
        let src_path = entry.path();
        if patterns.should_ignore(&src_path) {
            continue;
        }
        let dst_path = dst_dir.join(entry.file_name());

        if src_path.is_dir() && !is_symlink(&src_path) {
            // Recurse into real dirs
            if is_symlink(&dst_path) && is_stow_symlink(&dst_path, &config.stow_dir) {
                // Folded dir symlink pointing to our package — remove it
                if let Ok(tgt) = read_link_target(&dst_path) {
                    let abs_tgt = if tgt.is_absolute() {
                        tgt
                    } else {
                        dst_path.parent().unwrap_or(Path::new(".")).join(tgt)
                    };
                    let pkg_abs =
                        std::fs::canonicalize(&src_path).unwrap_or_else(|_| src_path.clone());
                    let abs_tgt = std::fs::canonicalize(&abs_tgt).unwrap_or(abs_tgt);
                    if pkg_abs == abs_tgt {
                        actions.push(Action::RemoveSymlink { path: dst_path });
                    }
                }
            } else if dst_path.is_dir() && !is_symlink(&dst_path) {
                plan_unstow_dir(&src_path, &dst_path, config, patterns, actions)?;
                // If dst_path will be empty after removals, schedule dir removal
                // (handled during execution)
            }
        } else if is_symlink(&dst_path) && is_stow_symlink(&dst_path, &config.stow_dir) {
            // Check it points to this package's file
            if let Ok(tgt) = read_link_target(&dst_path) {
                let abs_tgt = if tgt.is_absolute() {
                    tgt
                } else {
                    dst_path.parent().unwrap_or(Path::new(".")).join(tgt)
                };
                let src_abs = std::fs::canonicalize(&src_path).unwrap_or_else(|_| src_path.clone());
                let abs_tgt = std::fs::canonicalize(&abs_tgt).unwrap_or(abs_tgt);
                if src_abs == abs_tgt {
                    actions.push(Action::RemoveSymlink { path: dst_path });
                }
            }
        }
    }
    Ok(())
}

/// Execute a list of planned actions.
pub fn execute_actions(actions: &[Action], config: &Config) -> Result<(), StowError> {
    for action in actions {
        if config.verbose >= 1 {
            eprintln!("[bestow] {action}");
        }
        match action {
            Action::CreateSymlink { src, dst } => {
                if let Some(parent) = dst.parent()
                    && !parent.exists()
                {
                    std::fs::create_dir_all(parent).map_err(|e| StowError::Io {
                        path: parent.to_path_buf(),
                        source: e,
                    })?;
                }
                create_symlink(src, dst)?;
            }
            Action::CreateDir { path } => {
                std::fs::create_dir_all(path).map_err(|e| StowError::Io {
                    path: path.clone(),
                    source: e,
                })?;
            }
            Action::RemoveSymlink { path } => {
                if is_symlink(path) {
                    std::fs::remove_file(path).map_err(|e| StowError::Io {
                        path: path.clone(),
                        source: e,
                    })?;
                } else if path.is_file() {
                    // adopt: remove regular file (will be replaced by symlink)
                    std::fs::remove_file(path).map_err(|e| StowError::Io {
                        path: path.clone(),
                        source: e,
                    })?;
                }
            }
            Action::Unfold {
                dir,
                existing_link_target,
            } => {
                // Resolve the existing target dir
                let abs_target = if existing_link_target.is_absolute() {
                    existing_link_target.clone()
                } else {
                    dir.parent()
                        .unwrap_or(Path::new("."))
                        .join(existing_link_target)
                };

                // Remove the folded symlink
                std::fs::remove_file(dir).map_err(|e| StowError::Io {
                    path: dir.clone(),
                    source: e,
                })?;

                // Create a real directory
                std::fs::create_dir(dir).map_err(|e| StowError::Io {
                    path: dir.clone(),
                    source: e,
                })?;

                // Re-create individual symlinks for everything in the old target
                if abs_target.is_dir() {
                    for entry in std::fs::read_dir(&abs_target).map_err(|e| StowError::Io {
                        path: abs_target.clone(),
                        source: e,
                    })? {
                        let entry = entry.map_err(|e| StowError::Io {
                            path: abs_target.clone(),
                            source: e,
                        })?;
                        let inner_src = entry.path();
                        let inner_dst = dir.join(entry.file_name());
                        let inner_abs =
                            std::fs::canonicalize(&inner_src).unwrap_or_else(|_| inner_src.clone());
                        let rel = relative_path(&inner_dst, &inner_abs);
                        create_symlink(&rel, &inner_dst)?;
                    }
                }
            }
        }
    }

    // Clean up empty dirs in target after unstow
    Ok(())
}

/// Remove empty directories in `dir` that were created by stow (bottom-up).
/// Never removes `target_dir` itself, only subdirectories within it.
pub fn cleanup_empty_dirs(dir: &Path, target_dir: &Path) -> Result<(), StowError> {
    if !dir.is_dir() {
        return Ok(());
    }
    let entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| StowError::Io {
            path: dir.to_path_buf(),
            source: e,
        })?
        .filter_map(|e| e.ok())
        .collect();

    for entry in &entries {
        let p = entry.path();
        if p.is_dir() && !is_symlink(&p) {
            cleanup_empty_dirs(&p, target_dir)?;
        }
    }

    // Check if now empty (entries may have been removed by recursion)
    let is_empty = std::fs::read_dir(dir)
        .map(|mut d| d.next().is_none())
        .unwrap_or(false);
    if is_empty && dir != target_dir {
        std::fs::remove_dir(dir).map_err(|e| StowError::Io {
            path: dir.to_path_buf(),
            source: e,
        })?;
    }
    Ok(())
}

/// Collect all entries in a package dir via walkdir (for logging).
#[allow(dead_code)]
pub fn walk_package(package_dir: &Path, patterns: &Patterns) -> Vec<PathBuf> {
    WalkDir::new(package_dir)
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| !patterns.should_ignore(e.path()))
        .map(|e| e.path().to_path_buf())
        .collect()
}
