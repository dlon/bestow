use std::path::PathBuf;
use std::process;

use bestow::{Config, Operation, StowError, cli::Cli, ignore::Patterns, run};
use clap::Parser;

fn main() {
    let cli = Cli::parse();

    let stow_dir = cli
        .stow_dir
        .unwrap_or_else(|| std::env::current_dir().expect("cannot determine current directory"));
    let stow_dir = stow_dir.canonicalize().unwrap_or_else(|_| stow_dir.clone());

    let target_dir = cli.target.unwrap_or_else(|| {
        stow_dir
            .parent()
            .map(PathBuf::from)
            .expect("stow dir has no parent")
    });
    let target_dir = if target_dir.exists() {
        target_dir
            .canonicalize()
            .unwrap_or_else(|_| target_dir.clone())
    } else {
        target_dir
    };

    // Determine operation (default: stow)
    let operation = if cli.restow {
        Operation::Restow
    } else if cli.delete {
        Operation::Unstow
    } else {
        Operation::Stow
    };

    // Validate packages exist
    for pkg in &cli.packages {
        let pkg_path = stow_dir.join(pkg);
        if !pkg_path.is_dir() {
            eprintln!(
                "bestow: package not found: {pkg} (looked in {})",
                stow_dir.display()
            );
            process::exit(1);
        }
    }

    let config = Config {
        stow_dir,
        target_dir,
        operation,
        dry_run: cli.dry_run,
        adopt: cli.adopt,
        verbose: cli.verbose,
    };

    let patterns = match Patterns::new(&cli.ignore, &cli.defer, &cli.override_) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("bestow: invalid pattern: {e}");
            process::exit(1);
        }
    };

    if let Err(e) = run(&config, &cli.packages, &patterns) {
        match &e {
            StowError::Conflicts(cs) => {
                eprintln!("bestow: conflicts detected:\n{cs}");
            }
            other => {
                eprintln!("bestow: {other}");
            }
        }
        process::exit(1);
    }
}
