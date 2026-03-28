use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "bestow",
    version,
    about = "A symlink manager",
    long_about = "bestow manages symlink farms to install/uninstall packages (directories of files) \
                  into a target directory. bestow is based on GNU Stow.",
    arg_required_else_help = true
)]
pub struct Cli {
    /// Target directory where symlinks are created [default: parent of stow dir]
    #[arg(short, long, value_name = "DIR")]
    pub target: Option<PathBuf>,

    /// Stow directory containing packages [default: current directory]
    #[arg(short = 'd', long = "dir", value_name = "DIR")]
    pub stow_dir: Option<PathBuf>,

    /// Stow packages (default action)
    #[arg(short = 'S', long = "stow")]
    pub stow: bool,

    /// Unstow/delete packages
    #[arg(short = 'D', long = "delete")]
    pub delete: bool,

    /// Restow packages (unstow then stow)
    #[arg(short = 'R', long = "restow")]
    pub restow: bool,

    /// Dry run: simulate without making changes
    #[arg(short = 'n', long = "no")]
    pub dry_run: bool,

    /// Move existing target files into the package before stowing
    #[arg(long)]
    pub adopt: bool,

    /// Ignore files matching REGEX pattern (can be repeated)
    #[arg(long, value_name = "REGEX", action = clap::ArgAction::Append)]
    pub ignore: Vec<String>,

    /// Skip conflicts with already-stowed packages matching REGEX
    #[arg(long, value_name = "REGEX", action = clap::ArgAction::Append)]
    pub defer: Vec<String>,

    /// Force override of conflicts matching REGEX
    #[arg(long, value_name = "REGEX", action = clap::ArgAction::Append)]
    pub override_: Vec<String>,

    /// Verbose output (repeat for more: -vv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Packages to operate on
    #[arg(required = true, value_name = "PACKAGE")]
    pub packages: Vec<String>,
}
