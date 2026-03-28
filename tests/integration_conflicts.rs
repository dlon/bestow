use std::fs;
use std::path::Path;

use tempfile::TempDir;

use bestow::{Config, Operation, StowError, ignore::Patterns, run};

fn make_config(stow_dir: &Path, target_dir: &Path) -> Config {
    Config {
        stow_dir: stow_dir.to_path_buf(),
        target_dir: target_dir.to_path_buf(),
        operation: Operation::Stow,
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

#[test]
fn test_conflict_existing_file() {
    let tmp = TempDir::new().unwrap();
    let stow_dir = tmp.path().join("stow");
    let target_dir = tmp.path().join("target");
    fs::create_dir_all(&stow_dir).unwrap();
    fs::create_dir_all(&target_dir).unwrap();

    write_file(&stow_dir.join("pkg/file.txt"), "package version");
    // Pre-existing file in target
    write_file(&target_dir.join("file.txt"), "existing version");

    let config = make_config(&stow_dir, &target_dir);
    let result = run(&config, &["pkg".to_string()], &empty_patterns());

    assert!(
        matches!(result, Err(StowError::Conflicts(_))),
        "should report conflict"
    );
    // Original file should be untouched
    assert_eq!(
        fs::read_to_string(target_dir.join("file.txt")).unwrap(),
        "existing version"
    );
}

#[test]
fn test_adopt_resolves_conflict() {
    let tmp = TempDir::new().unwrap();
    let stow_dir = tmp.path().join("stow");
    let target_dir = tmp.path().join("target");
    fs::create_dir_all(&stow_dir).unwrap();
    fs::create_dir_all(&target_dir).unwrap();

    write_file(&stow_dir.join("pkg/file.txt"), "package version");
    write_file(&target_dir.join("file.txt"), "existing version");

    let config = Config {
        stow_dir: stow_dir.clone(),
        target_dir: target_dir.clone(),
        operation: Operation::Stow,
        dry_run: false,
        adopt: true,
        verbose: 0,
    };
    run(&config, &["pkg".to_string()], &empty_patterns()).unwrap();

    // After adopt, target/file.txt should be a symlink to pkg/file.txt
    let link = target_dir.join("file.txt");
    assert!(
        link.symlink_metadata()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false),
        "should be a symlink after adopt"
    );
}

#[test]
fn test_conflict_multiple_reported_at_once() {
    let tmp = TempDir::new().unwrap();
    let stow_dir = tmp.path().join("stow");
    let target_dir = tmp.path().join("target");
    fs::create_dir_all(&stow_dir).unwrap();
    fs::create_dir_all(&target_dir).unwrap();

    write_file(&stow_dir.join("pkg/a.txt"), "a");
    write_file(&stow_dir.join("pkg/b.txt"), "b");
    write_file(&target_dir.join("a.txt"), "existing a");
    write_file(&target_dir.join("b.txt"), "existing b");

    let config = make_config(&stow_dir, &target_dir);
    let result = run(&config, &["pkg".to_string()], &empty_patterns());

    match result {
        Err(StowError::Conflicts(cs)) => {
            assert!(cs.conflicts.len() >= 2, "should report both conflicts");
        }
        other => panic!("expected conflicts, got: {:?}", other),
    }
}

#[test]
fn test_defer_skips_conflict() {
    let tmp = TempDir::new().unwrap();
    let stow_dir = tmp.path().join("stow");
    let target_dir = tmp.path().join("target");
    fs::create_dir_all(&stow_dir).unwrap();
    fs::create_dir_all(&target_dir).unwrap();

    // Stow pkgA first
    write_file(&stow_dir.join("pkgA/bin/tool"), "tool");
    let config = Config {
        stow_dir: stow_dir.clone(),
        target_dir: target_dir.clone(),
        operation: Operation::Stow,
        dry_run: false,
        adopt: false,
        verbose: 0,
    };
    run(&config, &["pkgA".to_string()], &empty_patterns()).unwrap();

    // Now stow pkgB which would conflict — defer matching files
    write_file(&stow_dir.join("pkgB/bin/tool"), "tool from pkgB");
    let patterns = Patterns::new(&[], &["tool".to_string()], &[]).unwrap();
    let config = Config {
        stow_dir: stow_dir.clone(),
        target_dir: target_dir.clone(),
        operation: Operation::Stow,
        dry_run: false,
        adopt: false,
        verbose: 0,
    };
    // Should not error due to defer
    run(&config, &["pkgB".to_string()], &patterns).unwrap();
}
