# 🔧 Build Instructions

## Building Cross Cleaner

### Prerequisites

- **Rust** 1.70 or higher
- **Cargo** (included with Rust)
- **Git**

For Windows builds, you'll also need:
- Windows SDK (for resource compilation)

### Quick Start

```bash
# Clone the repository
git clone https://github.com/WinBooster/Cross-Cleaner.git
cd Cross-Cleaner

# Build debug version
cargo build

# Build release version (optimized)
cargo build --release
```

### Build Specific Components

```bash
# Build CLI only
cargo build --bin Cross_Cleaner_CLI --release

# Build GUI only
cargo build --bin Cross_Cleaner_GUI --release
```

## 📦 Version Management

### Setting Custom Version

The project uses the `APP_VERSION` environment variable to set the version for all binaries:

```bash
# Windows (PowerShell)
$env:APP_VERSION="1.9.6"
cargo build --release

# Windows (CMD)
set APP_VERSION=1.9.6
cargo build --release

# Linux/macOS
APP_VERSION="1.9.6" cargo build --release
```

### How Version System Works

1. **Environment Variable**: `APP_VERSION` is read during build
2. **CLI build.rs**: Sets Windows resource version info
3. **GUI build.rs**: Sets Windows resource version info + manifest
4. **Default Version**: If `APP_VERSION` is not set, defaults to "1.0.0"

### Updating Version in Cargo.toml Files

To update the version in all Cargo.toml files:

```bash
# Manually edit these files:
- cli/Cargo.toml
- gui/Cargo.toml
- database/Cargo.toml
- cleaner/Cargo.toml
```

Or use a script:

```bash
# Linux/macOS
find . -name "Cargo.toml" -type f -exec sed -i 's/version = "1.0.0"/version = "1.9.6"/g' {} +

# Windows (PowerShell)
Get-ChildItem -Recurse -Filter "Cargo.toml" | ForEach-Object {
    (Get-Content $_.FullName) -replace 'version = "1.0.0"', 'version = "1.9.6"' | Set-Content $_.FullName
}
```

## 🎯 Build Profiles

### Debug Profile

```bash
cargo build
```

- No optimizations
- Debug symbols included
- Faster compilation
- Larger binary size
- Use for development

### Release Profile

```bash
cargo build --release
```

Optimizations applied:
- **LTO (Link-Time Optimization)**: Enabled
- **Optimization Level**: `z` (optimize for size)
- **Code Generation Units**: 1 (better optimization)
- **Strip Symbols**: Enabled (smaller binary)
- **Panic Strategy**: `abort` (smaller binary)
- **Debug Info**: Disabled

Expected results:
- **CLI**: ~1.1 MB
- **GUI**: ~4.2 MB

## 📊 Database Optimization

The project automatically optimizes the JSON database at compile time:

1. **Minification**: Removes all whitespace from JSON
2. **Gzip Compression**: Compresses minified JSON

**Results:**
- Windows DB: 113 KB → 9 KB (92% reduction)
- Linux DB: 39 KB → 4 KB (89% reduction)
- **Total savings: 136 KB in final binary**

This happens automatically during build via `database/build.rs`.

## 🚀 Cross-Compilation

### Building for Windows from Linux

```bash
# Install mingw-w64
sudo apt-get install mingw-w64

# Add Windows target
rustup target add x86_64-pc-windows-gnu

# Build
cargo build --target x86_64-pc-windows-gnu --release
```

### Building for Linux from Windows (WSL)

```bash
# Use WSL (Windows Subsystem for Linux)
wsl
cd /mnt/c/path/to/Cross-Cleaner
cargo build --release
```

## 🔍 Troubleshooting

### Build Fails on Windows

**Problem**: Missing Windows SDK
```
Solution: Install Visual Studio Build Tools or Windows SDK
```

**Problem**: Icon not found
```
Solution: Ensure assets/icon.ico exists in the project root
```

### Build Fails with "Out of Memory"

```bash
# Reduce parallel jobs
cargo build --release -j 2
```

### Slow Build Times

```bash
# Use sccache for caching
cargo install sccache
export RUSTC_WRAPPER=sccache
```

## 📝 CI/CD Integration

### GitHub Actions Example

```yaml
- name: Build Release
  run: |
    APP_VERSION=${{ github.ref_name }} cargo build --release
```

### Manual Release Build

```bash
# Set version from git tag
export APP_VERSION=$(git describe --tags --abbrev=0)
cargo build --release

# Binaries location:
# - target/release/Cross_Cleaner_CLI
# - target/release/Cross_Cleaner_GUI
```

## 🧪 Testing

```bash
# Run all tests
cargo test

# Run tests for specific component
cargo test -p database
cargo test -p cleaner

# Run with verbose output
cargo test -- --nocapture
```

## 📦 Creating Distributable Package

```bash
# Build release
APP_VERSION="1.9.6" cargo build --release

# Windows: Binaries are in target/release/
# - Cross_Cleaner_CLI.exe
# - Cross_Cleaner_GUI.exe

# Optional: Strip and compress
strip target/release/Cross_Cleaner_*.exe
upx --best target/release/Cross_Cleaner_*.exe
```

## 🔧 Build Configuration Files

- `Cargo.toml` (root): Workspace configuration + release profile
- `cli/build.rs`: CLI build script (Windows resources)
- `gui/build.rs`: GUI build script (Windows resources + manifest)
- `database/build.rs`: Database optimization (minify + gzip)

## 💡 Tips

1. **First build is slow**: Rust compiles all dependencies
2. **Incremental builds are fast**: Only changed code recompiles
3. **Use `--release` for distribution**: Debug builds are much larger
4. **Check size**: `ls -lh target/release/Cross_Cleaner_*`
5. **Clean build**: `cargo clean` if you encounter issues

## 📚 Additional Resources

- [Rust Book](https://doc.rust-lang.org/book/)
- [Cargo Book](https://doc.rust-lang.org/cargo/)
- [Cross-Compilation Guide](https://rust-lang.github.io/rustup/cross-compilation.html)