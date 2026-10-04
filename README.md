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

### Demo
<img width="470" height="157" alt="image" src="https://github.com/user-attachments/assets/0cee6303-7ada-49f2-bd33-8159a583ebf9" />

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
