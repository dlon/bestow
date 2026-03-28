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

/// Check a path is a symlink
fn is_symlink(path: &Path) -> bool {
    path.symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

#[test]
fn test_basic_stow_single_package() {
    let tmp = TempDir::new().unwrap();
    let stow_dir = tmp.path().join("stow");
    let target_dir = tmp.path().join("target");
    fs::create_dir_all(&stow_dir).unwrap();
    fs::create_dir_all(&target_dir).unwrap();

    // Create package: pkg/bin/hello
    write_file(&stow_dir.join("pkg/bin/hello"), "hello");
    write_file(&stow_dir.join("pkg/.bashrc"), "# bashrc");

    let config = make_config(&stow_dir, &target_dir, Operation::Stow);
    run(&config, &["pkg".to_string()], &empty_patterns()).unwrap();

    // After stowing, target/bin should be a symlink (tree folding)
    let bin_link = target_dir.join("bin");
    assert!(
        is_symlink(&bin_link),
        "bin should be a dir symlink (folded)"
    );

    // .bashrc should be a symlink
    let bashrc = target_dir.join(".bashrc");
    assert!(is_symlink(&bashrc), ".bashrc should be symlinked");
}

#[test]
fn test_stow_creates_relative_symlinks() {
    let tmp = TempDir::new().unwrap();
    let stow_dir = tmp.path().join("stow");
    let target_dir = tmp.path().join("target");
    fs::create_dir_all(&stow_dir).unwrap();
    fs::create_dir_all(&target_dir).unwrap();

    write_file(&stow_dir.join("pkg/file.txt"), "content");

    let config = make_config(&stow_dir, &target_dir, Operation::Stow);
    run(&config, &["pkg".to_string()], &empty_patterns()).unwrap();

    let link = target_dir.join("file.txt");
    assert!(is_symlink(&link));
    let target = fs::read_link(&link).unwrap();
    // Should be a relative path, not absolute
    assert!(
        target.is_relative(),
        "symlink should be relative, got: {}",
        target.display()
    );
}

#[test]
fn test_stow_multiple_packages_tree_unfolding() {
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

    // bin should be a real dir (unfolded), with foo and bar as symlinks
    let bin = target_dir.join("bin");
    assert!(
        bin.is_dir() && !is_symlink(&bin),
        "bin should be a real dir after unfolding"
    );
    assert!(is_symlink(&bin.join("foo")), "foo should be a symlink");
    assert!(is_symlink(&bin.join("bar")), "bar should be a symlink");
}

#[test]
fn test_idempotent_restow() {
    let tmp = TempDir::new().unwrap();
    let stow_dir = tmp.path().join("stow");
    let target_dir = tmp.path().join("target");
    fs::create_dir_all(&stow_dir).unwrap();
    fs::create_dir_all(&target_dir).unwrap();

    write_file(&stow_dir.join("pkg/file.txt"), "content");

    let config = make_config(&stow_dir, &target_dir, Operation::Stow);
    run(&config, &["pkg".to_string()], &empty_patterns()).unwrap();

    // Stow again — should not error
    run(&config, &["pkg".to_string()], &empty_patterns()).unwrap();

    let link = target_dir.join("file.txt");
    assert!(is_symlink(&link));
}

#[test]
fn test_dry_run_no_changes() {
    let tmp = TempDir::new().unwrap();
    let stow_dir = tmp.path().join("stow");
    let target_dir = tmp.path().join("target");
    fs::create_dir_all(&stow_dir).unwrap();
    fs::create_dir_all(&target_dir).unwrap();

    write_file(&stow_dir.join("pkg/file.txt"), "content");

    let config = Config {
        stow_dir: stow_dir.clone(),
        target_dir: target_dir.clone(),
        operation: Operation::Stow,
        dry_run: true,
        adopt: false,
        verbose: 0,
    };
    run(&config, &["pkg".to_string()], &empty_patterns()).unwrap();

    // Nothing should have been created
    assert!(!target_dir.join("file.txt").exists());
}

#[test]
fn test_ignore_pattern() {
    let tmp = TempDir::new().unwrap();
    let stow_dir = tmp.path().join("stow");
    let target_dir = tmp.path().join("target");
    fs::create_dir_all(&stow_dir).unwrap();
    fs::create_dir_all(&target_dir).unwrap();

    write_file(&stow_dir.join("pkg/keep.txt"), "keep");
    write_file(&stow_dir.join("pkg/skip.log"), "skip");

    let patterns = Patterns::new(&["log$".to_string()], &[], &[]).unwrap();
    let config = make_config(&stow_dir, &target_dir, Operation::Stow);
    run(&config, &["pkg".to_string()], &patterns).unwrap();

    assert!(is_symlink(&target_dir.join("keep.txt")));
    assert!(!target_dir.join("skip.log").exists());
}
