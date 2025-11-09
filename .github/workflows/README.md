# Convert existing files to LF
dos2unix src/lib.rs
git config core.autocrlf false
git config core.eol lf
```

### 2. macOS/Linux - NAPI Linking Issues

**Problem**: `symbol(s) not found for architecture arm64` errors during linking

**Solution Implemented**:
- Added `NAPI_RS_LINK_TYPE=dynamic` environment variable
- Improved cross-compilation setup for ARM64 targets
- Updated NAPI configuration in `Cargo.toml` with async features
- Added proper SDK paths for macOS cross-compilation

**Environment Variables**:
```bash
export NAPI_RS_LINK_TYPE=dynamic
# For macOS ARM64 cross-compilation
export SDKROOT=$(xcrun --sdk macosx --show-sdk-path)
export MACOSX_DEPLOYMENT_TARGET=$(sw_vers -productVersion)
# For Linux ARM64 cross-compilation
export CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc
export CXX_aarch64_unknown_linux_gnu=aarch64-linux-gnu-g++
```

### 3. Dependency Configuration

**Problem**: Rodio feature configuration errors

**Solution Implemented**:
- Fixed rodio features from `ogg` to `vorbis`
- Added `async` feature to NAPI for better performance
- Optimized release profile settings

## Workflow Files

- `ci-windows.yml` - Windows-specific CI pipeline
- `ci-macos.yml` - macOS-specific CI pipeline  
- `ci-linux.yml` - Linux-specific CI pipeline
- `ci.yml` - Main CI coordination workflow
- `release.yml` - Release automation workflow
- `reusable-templates.yml` - Reusable workflow components

## Testing Locally

### Prerequisites
```bash
# Install dependencies
bun install

# Setup git hooks
./setup-hooks.sh
```

### Build Commands
```bash
# Debug build
bunx napi build --platform

# Release build
bunx napi build --platform --release

# Cross-platform build
bunx napi build --target x86_64-apple-darwin --release
bunx napi build --target aarch64-apple-darwin --release
```

### Quality Checks
```bash
# Format code
cargo fmt --all

# Check formatting
cargo fmt --all -- --check

# Run clippy
cargo clippy --all-targets --all-features -- -D warnings

# Run tests
cargo test --verbose
bun test
```

## Common Debugging Steps

### 1. Clean Build Environment
```bash
# Clean all build artifacts
cargo clean
napi clean
rm -rf target/
rm -rf node_modules/
bun install
```

### 2. Check Dependencies
```bash
# Check Cargo.lock consistency
cargo check

# Update dependencies if needed
cargo update
```

### 3. Verify Cross-Compilation Setup
```bash
# List installed Rust targets
rustup target list --installed

# Add missing targets
rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin
rustup target add x86_64-pc-windows-msvc
rustup target add aarch64-pc-windows-msvc
rustup target add x86_64-unknown-linux-gnu
rustup target add aarch64-unknown-linux-gnu
```

### 4. Check NAPI Configuration
```bash
# Verify NAPI configuration
cat .napirc.json
cat package.json | jq '.napi'
```

## Performance Optimizations

### Release Profile Optimizations
```toml
[profile.release]
lto = true              # Link-time optimization
codegen-units = 1       # Better optimization
panic = "abort"         # Smaller binary
strip = true           # Remove debug symbols
opt-level = "z"         # Size optimization
```

### NAPI-specific Optimizations
```toml
[profile.release.package.napi]
codegen-units = 1       # Optimize NAPI for better linking
```

## Environment Variables

### CI Environment
- `RUST_TOOLCHAIN: stable`
- `NODE_VERSION: "20"`
- `NAPI_RS_LINK_TYPE: dynamic`

### Cross-Compilation
- `SDKROOT` (macOS)
- `MACOSX_DEPLOYMENT_TARGET` (macOS)
- `CC_*` and `CXX_*` (Linux cross-compilation)
- `PKG_CONFIG_ALLOW_CROSS=1`
- `ALSA_NO_PKG_CONFIG=1` (Linux audio)

## Monitoring and Alerting

The workflows include:
- Artifact uploads for all successful builds
- Comprehensive error logging
- Cross-platform testing matrix
- Dependency caching for faster builds

## Future Improvements

1. **ARM64 Support**: Re-enable ARM64 builds once linking issues are fully resolved
2. **Automated Testing**: Add integration tests with actual audio files
3. **Performance Benchmarks**: Add performance regression testing
4. **Security Scanning**: Add dependency vulnerability scanning
5. **Documentation Generation**: Auto-generate API documentation from code

## Troubleshooting Checklist

Before opening an issue:

- [ ] Clean build environment (`cargo clean`, `napi clean`)
- [ ] Update dependencies (`cargo update`, `bun install`)
- [ ] Check formatting (`cargo fmt --all -- --check`)
- [ ] Run clippy (`cargo clippy -- -D warnings`)
- [ ] Verify line endings (should be LF)
- [ ] Check git hooks are installed (`./setup-hooks.sh`)
- [ ] Test locally with same Node.js/Rust versions as CI

## Getting Help

For additional support:
1. Check GitHub Issues for existing solutions
2. Review workflow logs for specific error details
3. Test with minimal reproduction case
4. Include environment details (OS, Rust version, Node.js version)