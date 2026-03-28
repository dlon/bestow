# bestow

**Disclaimer**: Nearly all code was AI generated.

A symlink farm manager, similar to GNU Stow. Works on Windows, macOS, and Linux.

bestow helps you manage dotfiles and software packages by creating symlinks in a target directory that point back into a stow directory. This lets you keep your configuration files organized in one place while making them appear in the locations your tools expect.

## Why bestow?

GNU Stow is a popular tool for managing symlink farms, but it only runs on Unix-like systems. bestow brings the same workflow to Windows users, while also working on macOS and Linux as a drop-in alternative.

## Install

```
git clone https://github.com/dlon/bestow
cd bestow; cargo install --path .
```

## Usage

```
bestow [OPTIONS] <PACKAGE>...
```

Packages are subdirectories inside the stow directory. By default, bestow stows packages (creates symlinks). Use `-D` to delete and `-R` to restow.

### Options

| Flag | Description |
|------|-------------|
| `-t`, `--target <DIR>` | Target directory where symlinks are created (default: parent of stow dir) |
| `-d`, `--dir <DIR>` | Stow directory containing packages (default: current directory) |
| `-S`, `--stow` | Stow packages (default action) |
| `-D`, `--delete` | Unstow/delete packages |
| `-R`, `--restow` | Restow packages (unstow then stow) |
| `-n`, `--no` | Dry run: simulate without making changes |
| `--adopt` | Move existing target files into the package before stowing |
| `--ignore <REGEX>` | Ignore files matching REGEX pattern (can be repeated) |
| `--defer <REGEX>` | Skip conflicts with already-stowed packages matching REGEX |
| `--override <REGEX>` | Force override of conflicts matching REGEX |
| `-v`, `--verbose` | Verbose output (repeat for more: `-vv`) |

### Example

Stow a `dotfiles` package from `~/stow` into `~`:

```
bestow -d ~/stow -t ~ dotfiles
```

## License

GPL-3.0
