use regex::Regex;
use std::path::Path;

/// Built-in ignore patterns matching GNU stow defaults.
static BUILTIN_IGNORE: &[&str] = &[
    r"^RCS$",
    r"^CVS$",
    r"^\.git$",
    r"^\.hg$",
    r"^\.svn$",
    r"\.orig$",
    r"\.rej$",
    r"~$",           // editor backup files
    r"^#.*#$",       // emacs autosave
    r"^\.#",         // emacs lockfiles
    r"^\.DS_Store$", // macOS metadata
];

pub struct Patterns {
    pub ignore: Vec<Regex>,
    pub defer: Vec<Regex>,
    pub override_: Vec<Regex>,
}

impl Patterns {
    pub fn new(
        ignore: &[String],
        defer: &[String],
        override_: &[String],
    ) -> Result<Self, regex::Error> {
        let mut ignore_pats: Vec<Regex> = BUILTIN_IGNORE
            .iter()
            .map(|p| Regex::new(p))
            .collect::<Result<_, _>>()?;
        for pat in ignore {
            ignore_pats.push(Regex::new(pat)?);
        }
        let defer_pats = defer
            .iter()
            .map(|p| Regex::new(p))
            .collect::<Result<_, _>>()?;
        let override_pats = override_
            .iter()
            .map(|p| Regex::new(p))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            ignore: ignore_pats,
            defer: defer_pats,
            override_: override_pats,
        })
    }

    pub fn should_ignore(&self, path: &Path) -> bool {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        self.ignore.iter().any(|re| re.is_match(name))
    }

    pub fn should_defer(&self, path: &Path) -> bool {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        self.defer.iter().any(|re| re.is_match(name))
    }

    pub fn should_override(&self, path: &Path) -> bool {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        self.override_.iter().any(|re| re.is_match(name))
    }
}
