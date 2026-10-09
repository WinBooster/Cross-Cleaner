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
# Build TUI only
cargo build -p tui --release

# Build GUI only
cargo build -p desktop --release

# Build CLI only
cargo build -p cli --release
```

### Disabling Updates (`self-update`)

The window (`desktop`), terminal (`tui`) and command line (`cli`) binaries check
GitHub for a newer release, announce it, and replace their own executable with
the download. All of that sits behind the `self-update` feature, which is **on
by default**:

```bash
# Default: builds with update notifications and self-update
cargo build --release -p desktop
cargo build --release -p tui
cargo build --release -p cli

# Without either — for distribution packages
cargo build --release -p desktop --no-default-features
cargo build --release -p tui --no-default-features
cargo build --release -p cli --no-default-features
```

`--no-default-features` turns off the whole update path, not just the
self-replacement:

- `crates/selfupdate` is not compiled, so `self-replace` drops out of the binary
- the GitHub version check never runs, so the app makes no network request
- no update notification is raised and no release page is offered

Everything else is untouched — cleaning, settings, the changelog and the program
lists all behave the same.

This is the switch a distribution package wants: the repository owns the
version, so the app must not advertise a newer one or ask the user to replace a
file the package manager owns. The trade-off is that packagers have to publish
an update themselves; the app will never tell a user that a newer release exists.

The flag is a Cargo feature rather than a `cfg` you set by hand, so the CI
release builds keep their current behaviour without passing anything, and
`cargo build --no-default-features` is the whole switch.

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
- crates/appcore/Cargo.toml
- crates/cleaner/Cargo.toml
- crates/cli/Cargo.toml
- crates/database/Cargo.toml
- crates/desktop/Cargo.toml
- crates/gui/Cargo.toml
- crates/selfupdate/Cargo.toml
- crates/tui/Cargo.toml
- crates/winicon/Cargo.toml
```

Every crate lives under `crates/` and is a workspace member, `crates/winicon`
included: it is a build dependency of the two Windows binaries anyway, and
listing it is what lets the tests guarding the icon actually run.

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
- **TUI**: ~4.16 MB
- **CLI**: ~4 MB
- **GUI**: ~11.6 MB

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
Solution: Ensure crates/winicon/assets/icon.ico exists — `winicon` owns the
icon, so neither build.rs passes a path to it any more
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
# - target/release/tui
# - target/release/desktop
# - target/release/cli
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
# - tui.exe
# - desktop.exe
# - cli.exe

# Optional: Strip and compress
strip target/release/tui.exe
upx --best target/release/tui.exe
```

## 🔧 Build Configuration Files

- `Cargo.toml` (root): Workspace configuration + release profile
- `crates/winicon`: Shared Windows resources (icon, version block, elevation manifest)
- `crates/desktop/build.rs`: Window build script (delegates to `winicon`)
- `crates/tui/build.rs`: Terminal build script (delegates to `winicon`)
- `crates/cli/build.rs`: Command line build script (delegates to `winicon`)
- `crates/appcore/build.rs`: UI sounds (gzips `assets/*.mp3` into `OUT_DIR`)
- `crates/gui/build.rs`: GUI build script (compresses the window icons)
- `crates/database/build.rs`: Database optimization (minify + gzip)

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