pub mod cli;
pub mod conflict;
pub mod ignore;
pub mod stow;
pub mod symlink;

use std::path::PathBuf;

use thiserror::Error;

use conflict::ConflictSet;
use ignore::Patterns;
use stow::{cleanup_empty_dirs, execute_actions, plan_stow, plan_unstow};

/// The operation to perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Stow,
    Unstow,
    Restow,
}

/// Configuration for a stow run.
#[derive(Debug)]
pub struct Config {
    pub stow_dir: PathBuf,
    pub target_dir: PathBuf,
    pub operation: Operation,
    pub dry_run: bool,
    pub adopt: bool,
    pub verbose: u8,
}

#[derive(Debug, Error)]
pub enum StowError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("package not found: {0}")]
    PackageNotFound(String),

    #[error("invalid regex pattern: {0}")]
    InvalidPattern(#[from] regex::Error),

    #[error("conflicts detected:\n{0}")]
    Conflicts(ConflictSet),
}

/// Run the stow operation for the given packages.
pub fn run(config: &Config, packages: &[String], patterns: &Patterns) -> Result<(), StowError> {
    match config.operation {
        Operation::Stow => do_stow(config, packages, patterns),
        Operation::Unstow => do_unstow(config, packages, patterns),
        Operation::Restow => {
            do_unstow(config, packages, patterns)?;
            do_stow(config, packages, patterns)
        }
    }
}

fn do_stow(config: &Config, packages: &[String], patterns: &Patterns) -> Result<(), StowError> {
    if config.dry_run {
        // In dry-run we just plan all packages and print (no real execution)
        let mut all_actions = Vec::new();
        let mut conflicts = ConflictSet::default();
        for pkg in packages {
            let actions = plan_stow(pkg, config, patterns, &mut conflicts)?;
            all_actions.extend(actions);
        }
        if !conflicts.is_empty() {
            return Err(StowError::Conflicts(conflicts));
        }
        for a in &all_actions {
            println!("[dry-run] {a}");
        }
        return Ok(());
    }

    // Process packages sequentially so each package's planning sees the real
    // filesystem state updated by the previous package's execution.
    for pkg in packages {
        let mut conflicts = ConflictSet::default();
        let actions = plan_stow(pkg, config, patterns, &mut conflicts)?;
        if config.verbose >= 2 {
            for a in &actions {
                eprintln!("[bestow] planned: {a}");
            }
        }
        if !conflicts.is_empty() {
            return Err(StowError::Conflicts(conflicts));
        }
        execute_actions(&actions, config)?;
    }
    Ok(())
}

fn do_unstow(config: &Config, packages: &[String], patterns: &Patterns) -> Result<(), StowError> {
    let mut all_actions = Vec::new();

    for pkg in packages {
        let actions = plan_unstow(pkg, config, patterns)?;
        if config.verbose >= 2 {
            for a in &actions {
                eprintln!("[bestow] planned: {a}");
            }
        }
        all_actions.extend(actions);
    }

    if config.dry_run {
        for a in &all_actions {
            println!("[dry-run] {a}");
        }
        return Ok(());
    }

    execute_actions(&all_actions, config)?;

    // Clean up empty dirs left behind
    cleanup_empty_dirs(&config.target_dir, &config.target_dir)?;

    Ok(())
}
