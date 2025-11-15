<div align="center">
<h1>🌟 Cross Cleaner 🌟</h1>

[![Rustlang](https://img.shields.io/static/v1?label=Made%20with&message=Rust&logo=rust&labelColor=e82833&color=b11522)](https://www.rust-lang.org)
[![Github License](https://img.shields.io/github/license/WinBooster/Cross-Cleaner?logo=mdBook)](https://github.com/WinBooster/Cross-Cleaner/blob/main/LICENSE)
[![Build Status](https://github.com/WinBooster/Cross-Cleaner/actions/workflows/dev_build.yml/badge.svg)](https://github.com/WinBooster/Cross-Cleaner/actions)
[![Download](https://img.shields.io/github/downloads/WinBooster/Cross-Cleaner/total)](https://github.com/WinBooster/Cross-Cleaner/releases)
[![GitHub Issues](https://img.shields.io/github/issues/WinBooster/Cross-Cleaner)](https://github.com/WinBooster/Cross-Cleaner/issues)
[![GitHub Stars](https://img.shields.io/github/stars/WinBooster/Cross-Cleaner?style=social)](https://github.com/WinBooster/Cross-Cleaner/stargazers)

<img src="assets/icon.png" alt="Cross Cleaner Logo" width="64"/>

### A powerful system cleanup tool written in Rust

</div>

## 📌 About the Project

**Cross Cleaner** is a high-performance tool for cleaning temporary files, cache, and other system "junk" from your computer. Built with Rust for optimal speed and reliability.

### Key Features

- ⚡ **Blazing Fast**: Optimized with `opt-level=3`, mimalloc allocator, and atomic operations for **10-50x faster** performance
- 🚀 **Multi-threaded**: Leverages rayon for parallel processing on multi-core systems
- 🔒 **Secure**: Carefully preserves critical system files
- 💻 **Cross-Platform**: Full support for [Windows](https://github.com/WinBooster/Cross-Cleaner/blob/main/LIST_WINDOWS.md), [MacOS](https://github.com/WinBooster/Cross-Cleaner/blob/main/MACOS_WINDOWS.md) and [Linux](https://github.com/WinBooster/Cross-Cleaner/blob/main/LIST_LINUX.md)
- 🎯 **User-Friendly**: Clean, minimalist interface for easy operation
- 📄 **Custom-DataBase**: Ability to use custom cleanup database
- 🧪 **Well-Tested**: 59+ unit tests, 800+ property-based tests, and 9 performance benchmarks

### Demo
![CLI](https://github.com/user-attachments/assets/7d28a763-97ee-45b9-9ad5-2ed0fb8886c0)
<img width="430" height="168" alt="изображение" src="https://github.com/user-attachments/assets/4d4caa7c-15d7-4e8a-b547-f845d84dcea7" />



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

## 🧪 Testing & Benchmarking

Cross Cleaner has comprehensive test coverage to ensure reliability and performance.

### Run Tests
```bash
# Run all tests
cargo test --all

# Run property-based tests (800+ test cases)
cargo test --package cleaner proptests

# Run with more test cases
PROPTEST_CASES=1000 cargo test --package cleaner proptests
```

### Run Benchmarks
```bash
# Run performance benchmarks
cargo bench --package Cross_Cleaner_CLI

# View detailed HTML reports
open target/criterion/report/index.html
```

### Test Coverage
- **59 unit/integration tests** - Core functionality
- **800+ property-based tests** - Edge case detection with proptest
- **9 performance benchmarks** - Regression detection with criterion

See [TESTING.md](TESTING.md) for detailed testing documentation.
