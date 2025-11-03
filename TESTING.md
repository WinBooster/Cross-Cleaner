# 🧪 Testing Documentation

## Overview

This document describes the testing infrastructure for Cross Cleaner, including how to run tests, write new tests, and understand test coverage.

## Test Structure

```
Cross-Cleaner/
├── database/
│   └── src/
│       └── lib.rs              # Unit tests for database module
├── cleaner/
│   └── src/
│       └── lib.rs              # Unit tests for cleaner module
├── cli/
│   └── tests/
│       └── integration_tests.rs # Integration tests for CLI
└── gui/
    └── src/
        └── main.rs             # Unit tests for GUI (basic functions only)
```

## Running Tests

### Run All Tests

```bash
# Run all tests in the workspace
cargo test

# Run with output visible
cargo test -- --nocapture

# Run with verbose output
cargo test -- --show-output
```

### Run Tests for Specific Components

```bash
# Database tests only
cargo test -p database

# Cleaner tests only
cargo test -p cleaner

# CLI integration tests only
cargo test -p Cross_Cleaner_CLI
```

### Run Specific Test

```bash
# Run a specific test by name
cargo test test_get_version

# Run tests matching a pattern
cargo test database
```

# Run Tests in Release Mode

```bash
# Faster execution for performance tests
cargo test --release
```

### Important: GUI Tests and Admin Requirements

**Note:** GUI tests require admin privileges on Windows (debug builds only work without admin after build script changes).

```bash
# Run GUI tests (may require admin on Windows)
cargo test -p Cross_Cleaner_GUI --bin Cross_Cleaner_GUI

# Skip GUI tests
cargo test --workspace --exclude Cross_Cleaner_GUI
```

## Test Categories

### 1. Unit Tests (Database Module)

Located in: `database/src/lib.rs`

**Coverage:**
- ✅ Version management (`test_get_version`)
- ✅ Icon loading (`test_get_icon`)
- ✅ Database loading (`test_get_default_database`)
- ✅ Gzip decompression (`test_database_decompression`)
- ✅ Placeholder expansion (`test_database_placeholder_expansion`)
- ✅ File loading (`test_database_from_file_*`)
- ✅ File size formatting (`test_file_size_string_formatting`)
- ✅ Data structure validation (`test_cleaner_data_structure`)
- ✅ Database integrity (`test_database_no_duplicate_entries`)
- ✅ Performance (`test_database_performance`)

**Example:**
```bash
cargo test -p database test_get_version
```

### 2. Unit Tests (Cleaner Module)

Located in: `cleaner/src/lib.rs`

**Coverage:**
- ✅ Non-existent paths (`test_clear_data_nonexistent_path`)
- ✅ File removal (`test_clear_data_remove_files`)
- ✅ Directory removal (`test_clear_data_remove_directory`)
- ✅ Recursive deletion (`test_clear_data_remove_all_in_dir`)
- ✅ Specific file patterns (`test_clear_data_specific_files`)
- ✅ Specific directory patterns (`test_clear_data_specific_directories`)
- ✅ Glob patterns (`test_clear_data_glob_pattern`)
- ✅ Nested directories (`test_clear_data_nested_directories`)
- ✅ Byte counting (`test_clear_data_byte_counting`)
- ✅ Multiple operations (`test_clear_data_multiple_operations`)

**Example:**
```bash
cargo test -p cleaner test_clear_data_remove_files
```

### 3. Integration Tests (CLI)

Located in: `cli/tests/integration_tests.rs`

**Coverage:**
- ✅ CLI help (`test_cli_help`)
- ✅ CLI version (`test_cli_version`)
- ✅ Invalid database path (`test_cli_with_invalid_database_path`)
- ✅ Argument parsing (all flags tested)
- ✅ Custom database loading
- ✅ Category/program parsing

**Example:**
```bash
cargo test -p Cross_Cleaner_CLI test_cli_help
```

### 4. GUI Tests (Limited)

Located in: `gui/src/main.rs`

**Coverage:**
- ✅ Icon loading (`test_load_icon_from_bytes`)
- ✅ MyApp initialization (`test_myapp_from_database`)
- ✅ Category sorting (`test_myapp_category_sorting`)
- ✅ Args parsing (`test_args_parsing`)
- ✅ Initial state validation (`test_myapp_initial_state`)

**Note:** GUI tests are limited because:
- UI code cannot be easily unit tested
- GUI requires admin privileges on Windows (release builds)
- Focus is on testable business logic

**Example:**
```bash
# May require admin privileges on Windows
cargo test -p Cross_Cleaner_GUI --bin Cross_Cleaner_GUI
```

## Writing New Tests

### Database Tests

Add tests to `database/src/lib.rs`:

```rust
#[test]
fn test_my_feature() {
    let database = get_default_database();
    assert!(!database.is_empty());
    // Your test logic here
}
```

### Cleaner Tests

Add tests to `cleaner/src/lib.rs`:

```rust
#[test]
fn test_my_cleaner_feature() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.txt");
    fs::write(&file_path, b"test").unwrap();
    
    let mut data = create_test_data(file_path.to_str().unwrap().to_string());
    data.remove_files = true;
    
    let result = clear_data(&data);
    assert!(result.working);
}
```

### CLI Integration Tests

Add tests to `cli/tests/integration_tests.rs`:

```rust
#[test]
fn test_my_cli_feature() {
    let output = Command::new("cargo")
        .args(&["run", "--bin", "Cross_Cleaner_CLI", "--", "--help"])
        .output()
        .expect("Failed to execute command");
    
    assert!(output.status.success());
}
```

## Test Best Practices

### 1. Use Descriptive Names

✅ Good:
```rust
#[test]
fn test_database_loads_successfully_with_valid_json()
```

❌ Bad:
```rust
#[test]
fn test1()
```

### 2. Test One Thing at a Time

✅ Good:
```rust
#[test]
fn test_file_removal() {
    // Test only file removal
}

#[test]
fn test_directory_removal() {
    // Test only directory removal
}
```

❌ Bad:
```rust
#[test]
fn test_everything() {
    // Tests file removal, directory removal, byte counting, etc.
}
```

### 3. Use Temporary Files/Directories

Always use `tempfile::TempDir` for tests that manipulate the filesystem:

```rust
use tempfile::TempDir;

#[test]
fn test_with_temp_files() {
    let temp_dir = TempDir::new().unwrap();
    // temp_dir is automatically cleaned up when it goes out of scope
}
```

### 4. Assert Meaningful Conditions

✅ Good:
```rust
assert!(result.working, "Cleaner should report working status");
assert_eq!(result.files, 3, "Should have removed exactly 3 files");
```

❌ Bad:
```rust
assert!(true);
```

### 5. Handle Expected Failures

```rust
#[test]
fn test_invalid_input_returns_error() {
    let result = get_database_from_file("nonexistent.json");
    assert!(result.is_err(), "Should return error for non-existent file");
}
```

## Continuous Integration

### GitHub Actions

Tests run automatically on:
- Every push to main branch
- Every pull request
- Release builds

### Local Pre-commit Testing

Before committing, run:

```bash
# Full test suite
cargo test --all

# With formatting check
cargo fmt --check && cargo test --all

# With clippy lints
cargo clippy -- -D warnings && cargo test --all
```

## Test Coverage

### Current Coverage

| Module | Tests | Coverage |
|--------|-------|----------|
| Database | 15 tests | Core functionality ✅ |
| Cleaner | 12 tests | File operations ✅ |
| CLI | 10 tests | Arguments & Integration ✅ |
| GUI | 5 tests | Basic functions ✅ (UI not testable) |

### Measuring Coverage

Install cargo-tarpaulin:

```bash
cargo install cargo-tarpaulin
```

Run coverage:

```bash
# Generate coverage report
cargo tarpaulin --out Html

# Open coverage report
# Open tarpaulin-report.html in browser
```

## Performance Tests

Some tests measure performance:

```rust
#[test]
fn test_database_performance() {
    use std::time::Instant;
    
    let start = Instant::now();
    let database = get_default_database();
    let duration = start.elapsed();
    
    assert!(duration.as_millis() < 100, "Should load in < 100ms");
}
```

Run only performance tests:

```bash
cargo test performance
```

## Troubleshooting

### GUI Tests Fail with "Запрошенная операция требует повышения" (Requires elevation)

**Problem:** GUI tests require admin privileges on Windows in release builds.

**Solution 1:** Run PowerShell as Administrator:
```bash
# Run PowerShell as Administrator
cargo test
```

**Solution 2:** Skip GUI tests:
```bash
# Test everything except GUI
cargo test --workspace --exclude Cross_Cleaner_GUI
```

**Solution 3:** Build script modification (already done):
- Debug builds no longer require admin
- Only release builds require admin

### Tests Fail Due to Permissions

**Windows:** Run tests as Administrator if testing system cleanup:

```bash
# Run PowerShell as Administrator
cargo test
```

**Linux/macOS:** Use sudo if needed:

```bash
sudo cargo test
```

### Tests Timeout

Increase timeout:

```bash
cargo test -- --test-threads=1 --nocapture
```

### Clean Test Artifacts

```bash
# Remove all test artifacts
cargo clean

# Remove only test binaries
rm -rf target/debug/deps/*test*
```

### Flaky Tests

If tests fail intermittently:

1. Check for race conditions
2. Ensure proper cleanup with `TempDir`
3. Avoid relying on timing
4. Run multiple times:

```bash
# Run test 10 times
for i in {1..10}; do cargo test test_name || break; done
```

## Adding Tests to CI/CD

### GitHub Actions Example

```yaml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: Run tests
        run: cargo test --all --verbose
```

## Test Documentation

### Documenting Test Purpose

```rust
/// Tests that the database correctly loads and decompresses gzip data.
/// 
/// This test verifies:
/// - Gzip decompression works correctly
/// - JSON parsing succeeds
/// - Database contains expected number of entries
#[test]
fn test_database_decompression() {
    // Test implementation
}
```

## Quick Reference

| Command | Description |
|---------|-------------|
| `cargo test` | Run all tests |
| `cargo test -p database` | Test database module |
| `cargo test -p cleaner` | Test cleaner module |
| `cargo test -p Cross_Cleaner_CLI` | Test CLI |
| `cargo test -p Cross_Cleaner_GUI --bin Cross_Cleaner_GUI` | Test GUI (may need admin) |
| `cargo test --workspace --exclude Cross_Cleaner_GUI` | Test all except GUI |
| `cargo test -- --nocapture` | Show print output |
| `cargo test test_name` | Run specific test |
| `cargo test --release` | Run tests in release mode |
| `cargo test -- --test-threads=1` | Run tests serially |

## Contributing Tests

When contributing:

1. ✅ Add tests for new features
2. ✅ Update tests for modified features
3. ✅ Ensure all tests pass: `cargo test --all`
4. ✅ Follow naming conventions
5. ✅ Document test purpose
6. ✅ Use temporary files for filesystem tests
7. ✅ Clean up resources properly

## Future Testing Goals

- [x] Add GUI tests (basic functions) ✅
- [ ] Increase code coverage to 90%+
- [ ] Add benchmark tests
- [ ] Add property-based tests with `proptest`
- [ ] Add fuzzing tests with `cargo-fuzz`
- [ ] Add mutation testing with `cargo-mutants`
- [ ] Add E2E GUI tests with UI testing framework

## Resources

- [Rust Book - Testing](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Cargo Book - Tests](https://doc.rust-lang.org/cargo/guide/tests.html)
- [tempfile crate](https://docs.rs/tempfile/)
- [assert_cmd crate](https://docs.rs/assert_cmd/) - Better CLI testing

## Summary

✅ **42 tests** covering core functionality  
✅ **Automated testing** in CI/CD  
✅ **Fast execution** (< 10 seconds for full suite)  
✅ **Safe testing** with temporary files  
✅ **Easy to run** with `cargo test`  
✅ **Admin handling** for GUI tests on Windows

### Test Count Breakdown:
- Database: 15 tests
- Cleaner: 12 tests  
- CLI: 10 tests
- GUI: 5 tests
- **Total: 42 tests**

For questions or issues with tests, please open an issue on GitHub.