use std::fs;
use std::path::Path;

use tempfile::TempDir;

use bestow::{Config, Operation, ignore::Patterns, run};

fn make_config(stow_dir: &Path, target_dir: &Path, op: Operation) -> Config {
    Config {
        stow_dir: stow_dir.to_path_buf(),
        target_dir: target_dir.to_path_buf(),
        operation: op,
        dry_run: false,
        adopt: false,
        verbose: 0,
    }
}

fn empty_patterns() -> Patterns {
    Patterns::new(&[], &[], &[]).unwrap()
}

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

fn is_symlink(path: &Path) -> bool {
    path.symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

#[test]
fn test_basic_unstow() {
    let tmp = TempDir::new().unwrap();
    let stow_dir = tmp.path().join("stow");
    let target_dir = tmp.path().join("target");
    fs::create_dir_all(&stow_dir).unwrap();
    fs::create_dir_all(&target_dir).unwrap();

    write_file(&stow_dir.join("pkg/file.txt"), "content");

    // Stow first
    let config = make_config(&stow_dir, &target_dir, Operation::Stow);
    run(&config, &["pkg".to_string()], &empty_patterns()).unwrap();
    assert!(is_symlink(&target_dir.join("file.txt")));

    // Unstow
    let config = make_config(&stow_dir, &target_dir, Operation::Unstow);
    run(&config, &["pkg".to_string()], &empty_patterns()).unwrap();
    assert!(!target_dir.join("file.txt").exists());
}

#[test]
fn test_unstow_cleans_empty_dirs() {
    let tmp = TempDir::new().unwrap();
    let stow_dir = tmp.path().join("stow");
    let target_dir = tmp.path().join("target");
    fs::create_dir_all(&stow_dir).unwrap();
    fs::create_dir_all(&target_dir).unwrap();

    // Two packages share bin/
    write_file(&stow_dir.join("pkgA/bin/foo"), "foo");
    write_file(&stow_dir.join("pkgB/bin/bar"), "bar");

    let config = make_config(&stow_dir, &target_dir, Operation::Stow);
    run(
        &config,
        &["pkgA".to_string(), "pkgB".to_string()],
        &empty_patterns(),
    )
    .unwrap();

    // Unstow both
    let config = make_config(&stow_dir, &target_dir, Operation::Unstow);
    run(
        &config,
        &["pkgA".to_string(), "pkgB".to_string()],
        &empty_patterns(),
    )
    .unwrap();

    // bin dir should be cleaned up
    assert!(
        !target_dir.join("bin").exists(),
        "empty bin dir should be removed"
    );
}

#[test]
fn test_restow_after_adding_file() {
    let tmp = TempDir::new().unwrap();
    let stow_dir = tmp.path().join("stow");
    let target_dir = tmp.path().join("target");
    fs::create_dir_all(&stow_dir).unwrap();
    fs::create_dir_all(&target_dir).unwrap();

    write_file(&stow_dir.join("pkg/bin/original"), "orig");

    let config = make_config(&stow_dir, &target_dir, Operation::Stow);
    run(&config, &["pkg".to_string()], &empty_patterns()).unwrap();

    // Add a new file to the package
    write_file(&stow_dir.join("pkg/bin/new_file"), "new");

    // Restow
    let config = make_config(&stow_dir, &target_dir, Operation::Restow);
    run(&config, &["pkg".to_string()], &empty_patterns()).unwrap();

    // The bin dir may be folded (a dir symlink) or unfolded. Either way, new_file
    // must be accessible through the target.
    let new_file_path = target_dir.join("bin/new_file");
    assert!(
        new_file_path.exists(),
        "new_file should be accessible after restow"
    );
    // If bin is a folded dir symlink, bin itself is the symlink; if unfolded,
    // each file is a symlink.
    let bin_path = target_dir.join("bin");
    assert!(
        is_symlink(&bin_path) || is_symlink(&new_file_path),
        "either bin or new_file should be a symlink"
    );
}

#[test]
fn test_unstow_only_own_package_links() {
    let tmp = TempDir::new().unwrap();
    let stow_dir = tmp.path().join("stow");
    let target_dir = tmp.path().join("target");
    fs::create_dir_all(&stow_dir).unwrap();
    fs::create_dir_all(&target_dir).unwrap();

    write_file(&stow_dir.join("pkgA/bin/foo"), "foo");
    write_file(&stow_dir.join("pkgB/bin/bar"), "bar");

    let config = make_config(&stow_dir, &target_dir, Operation::Stow);
    run(
        &config,
        &["pkgA".to_string(), "pkgB".to_string()],
        &empty_patterns(),
    )
    .unwrap();

    // Unstow only pkgA
    let config = make_config(&stow_dir, &target_dir, Operation::Unstow);
    run(&config, &["pkgA".to_string()], &empty_patterns()).unwrap();

    // pkgA's file should be gone; pkgB's should remain
    assert!(!target_dir.join("bin/foo").exists());
    assert!(is_symlink(&target_dir.join("bin/bar")));
}
