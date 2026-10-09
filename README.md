<div align="center">
<h1>🌟 Cross Cleaner 🌟</h1>
<br>
An addon-style <a href="https://github.com/WinBooster/Cross-Cleaner">system cleanup tool</a> that removes temporary files, cache and other system junk from your computer.
<br>
<a href="https://www.rust-lang.org"><img src="https://img.shields.io/static/v1?label=Made%20with&message=Rust&logo=rust&labelColor=e82833&color=b11522" alt="Made with Rust"></a>
<a href="https://github.com/WinBooster/Cross-Cleaner/blob/main/LICENSE"><img src="https://img.shields.io/github/license/WinBooster/Cross-Cleaner?logo=mdBook" alt="Github License"></a>
<br>
<a href="https://github.com/WinBooster/Cross-Cleaner/actions"><img src="https://github.com/WinBooster/Cross-Cleaner/actions/workflows/dev_build.yml/badge.svg" alt="Build Status"></a>
<a href="https://github.com/WinBooster/Cross-Cleaner/releases"><img src="https://img.shields.io/github/downloads/WinBooster/Cross-Cleaner/total" alt="Downloads"/></a>
<a href="https://github.com/WinBooster/Cross-Cleaner/issues"><img src="https://img.shields.io/github/issues/WinBooster/Cross-Cleaner" alt="GitHub Issues"/></a>
<a href="https://github.com/WinBooster/Cross-Cleaner/stargazers"><img src="https://badgen.net/github/stars/WinBooster/Cross-Cleaner" alt="GitHub Stars"/></a>
<br>
<a href="https://discord.gg/wmJdUBaztX"><img src="https://img.shields.io/badge/support/help/issues-discord-brightgreen" alt="Discord"/></a>
<br>
<p>Join our Discord server for support, updates and community discussions 🤫</p>
</div>

## 📌 About the Project

**Cross Cleaner** is a high-performance tool for cleaning temporary files, cache, and other system "junk" from your computer. Built with Rust for optimal speed and reliability.

### Key Features

- 🚀 **Multi-threaded**: Leverages rayon for parallel processing on multi-core systems
- 🔒 **Secure**: Carefully preserves critical system files
- 💻 **Cross-Platform**: Full support for [Windows](https://github.com/WinBooster/Cross-Cleaner/blob/main/LIST_WINDOWS.md), [MacOS](https://github.com/WinBooster/Cross-Cleaner/blob/main/LIST_MACOS.md), [Linux](https://github.com/WinBooster/Cross-Cleaner/blob/main/LIST_LINUX.md) and [Android](https://github.com/WinBooster/Cross-Cleaner/blob/main/LIST_ANDROID.md)
- 🎯 **User-Friendly**: Clean, minimalist interface for easy operation
- 📄 **Custom-DataBase**: Ability to use custom cleanup database

### Demo Desktop Edition
<img width="570" height="557" alt="image" src="https://github.com/user-attachments/assets/69cc39e7-3824-4448-884f-cef4428ff731" />

### Demo TUI Edition
<img width="979" height="512" alt="image" src="https://github.com/user-attachments/assets/220db588-a366-4ba1-8101-04ea294a407a" />

## 🖥️ Command Line Edition

The same cleaner, driven from a terminal. It shares the database, the
selection rules and the cleaning engine with the two apps above — the only
thing that changes is how the selection is made.

```bash
# What can be cleaned?
cli categories
cli programs -c Cache

# What would a selection touch, without touching it?
cli plan -c Cache -c Logs --paths

# Clean it.
cli clean -c Cache -c Logs
cli clean --all                      # everything, like the window app's "clean all"
cli clean -c "Cache/Browser"         # one subcategory
cli clean -p Chrome                  # one program, all of its categories
cli clean -p "Chrome=Logs"           # one program, one of its categories
cli clean -a -e Discord              # everything except one program

# See what it would actually free, without deleting anything.
cli clean -c Cache --dry-run

# For a script.
cli clean -a --json | jq .bytes
```

Selection is as detailed as in the terminal app — a category, a subcategory of
one, a whole program, or a program narrowed to some of its categories. Names are
matched case-insensitively, and a name that does not exist is an error with the
closest real names next to it rather than a run that quietly cleans something
else.

`--dry-run` walks the same code the cleaner itself uses, so it measures rather
than estimates. Files another program is holding open are counted separately
instead of being promised as free, and cleaners that cannot be measured without
running (custom cleaners, registry entries) are named as such:

```text
$ cli clean -c Browser --dry-run
Would free 0 bytes in 0 files and 0 directories across 1 program

  Vivaldi  0 B  0 files · 0 directories

7 files are in use and will not be removed:
  7.6 MB held back — close the programs using them and run again.
```

The walk costs about as much I/O as the run does, so a full `--all` scan is not
instant. That is the price of an honest number.

| Command | What it does |
|---|---|
| `cli categories` | Categories with their subcategories and entry counts |
| `cli programs [-c CAT]` | Programs of the selected categories, with a search filter |
| `cli plan [selection]` | Which database patterns a selection covers, with `--paths` |
| `cli clean [selection]` | Clean it, or measure it with `--dry-run` |
| `cli update` | Check for and install a newer release |

Common flags: `-a/--all`, `-c/--category`, `-x/--exclude-category`,
`-p/--program`, `-e/--exclude-program`, `-y/--yes`, `-v/--verbose`,
`-q/--quiet`, `--json`, `--no-color`, `--dry-run`.

Exit codes: `0` success, `1` a failure, `130` interrupted with Ctrl-C.

## 📥 Installation

### Option 1: Download Pre-built Binary
Get the latest release from our [releases page](https://github.com/WinBooster/Cross-Cleaner/releases).

### Option 2: Build from Source

1. Make sure you have [Rust](https://www.rust-lang.org/) installed (version 1.70 or higher):
```bash
rustc --version
```

2. Clone the repository:
```bash
git clone https://github.com/WinBooster/Cross-Cleaner.git
cd Cross-Cleaner
```

3. Build the project:
```bash
cargo build --release
```

4. The compiled binary will be located in `target/release`

## 🤝 Contributing

Pull requests are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for build
checks, commit message conventions (they end up in the release notes) and the
maintainer release process.
