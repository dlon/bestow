use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConflictKind {
    #[error("target already exists and is not a stow symlink")]
    ExistingFile,
    #[error("target is a symlink owned by a different stowed package")]
    OwnedByOther { owner: PathBuf },
}

#[derive(Debug)]
pub struct Conflict {
    pub target: PathBuf,
    pub kind: ConflictKind,
}

#[derive(Debug, Default)]
pub struct ConflictSet {
    pub conflicts: Vec<Conflict>,
}

impl ConflictSet {
    pub fn add(&mut self, target: PathBuf, kind: ConflictKind) {
        self.conflicts.push(Conflict { target, kind });
    }

    pub fn is_empty(&self) -> bool {
        self.conflicts.is_empty()
    }
}

impl std::fmt::Display for ConflictSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for c in &self.conflicts {
            writeln!(f, "  conflict: {} — {}", c.target.display(), c.kind)?;
        }
        Ok(())
    }
}
