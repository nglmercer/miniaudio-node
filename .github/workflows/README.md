# GitHub Actions Workflows

This directory contains the modularized CI/CD workflows for the miniaudio-node project.

## Workflow Structure

### Main Workflows

#### `ci.yml` - Main CI/CD Pipeline
**Triggers**: Push/PR to `main` or `develop` branches

**Jobs**:
- `trigger-platform-cis`: Orchestrates platform-specific CI workflows
- `security`: Runs security audits (npm and cargo)
- `docs`: Deploys documentation (main branch only)
- `ci-summary`: Provides summary of all CI results

#### `release.yml` - Release and Publish
**Triggers**: Release publication

**Jobs**:
- `wait-for-builds`: Ensures platform builds complete
- `release`: Creates release assets and publishes to npm

### Platform-Specific Workflows

#### `ci-linux.yml` - Linux CI
**Triggers**: Push/PR to `main` or `develop` branches

**Jobs**:
- `rust-tests-linux`: Rust formatting, linting, and tests on Ubuntu
- `node-tests-linux`: Node.js/Bun tests on Ubuntu (Node 18 & 20)
- `build-linux`: Cross-compiles Linux binaries (x64-gnu, x64-musl, arm64-gnu)

#### `ci-macos.yml` - macOS CI
**Triggers**: Push/PR to `main` or `develop` branches

**Jobs**:
- `rust-tests-macos`: Rust formatting, linting, and tests on macOS
- `node-tests-macos`: Node.js/Bun tests on macOS (Node 18 & 20)
- `build-macos`: Cross-compiles macOS binaries (x64, arm64)

#### `ci-windows.yml` - Windows CI
**Triggers**: Push/PR to `main` or `develop` branches

**Jobs**:
- `rust-tests-windows`: Rust formatting, linting, and tests on Windows
- `node-tests-windows`: Node.js/Bun tests on Windows (Node 18 & 20)
- `build-windows`: Cross-compiles Windows binaries (x64-msvc, ia32-msvc, arm64-msvc)

### Reusable Components

#### `reusable-templates.yml` - Shared Templates
**Purpose**: Provides reusable workflow components for common setup tasks

**Jobs**:
- `setup-rust`: Configures Rust environment with system dependencies
- `setup-node`: Configures Node.js/Bun environment

## Key Improvements

### 1. Modular Architecture
- Separate workflows for each platform
- Clear separation of concerns
- Independent execution and failure isolation

### 2. ALSA Dependency Fix
- Linux workflows install `libasound2-dev` and `libpkgconf-dev`
- Resolves build failures on Ubuntu runners
- Only affects Linux builds

### 3. Improved Release Process
- Better artifact management and validation
- Comprehensive package validation
- Release asset creation
- Robust npm publishing

### 4. Enhanced Error Handling
- Continue-on-error for artifact downloads
- Validation steps for release packages
- Clear error messages and debugging info

### 5. Optimized Caching
- Platform-specific cache keys
- Separate caches for Rust and Node dependencies
- Better cache hit rates

## Binary Targets

### Linux
- `x86_64-unknown-linux-gnu` → `miniaudio_node.linux-x64-gnu.node`
- `x86_64-unknown-linux-musl` → `miniaudio_node.linux-x64-musl.node`
- `aarch64-unknown-linux-gnu` → `miniaudio_node.linux-arm64-gnu.node`

### macOS
- `x86_64-apple-darwin` → `miniaudio_node.darwin-x64.node`
- `aarch64-apple-darwin` → `miniaudio_node.darwin-arm64.node`

### Windows
- `x86_64-pc-windows-msvc` → `miniaudio_node.win32-x64-msvc.node`
- `i686-pc-windows-msvc` → `miniaudio_node.win32-ia32-msvc.node`
- `aarch64-pc-windows-msvc` → `miniaudio_node.win32-arm64-msvc.node`

## Environment Variables

### Global
- `RUST_TOOLCHAIN`: Stable Rust toolchain
- `NODE_VERSION`: Node.js version 20

### Secrets Required
- `NPM_TOKEN`: For npm package publishing
- `GITHUB_TOKEN`: For release asset uploads (automatic)

## Troubleshooting

### ALSA Issues (Linux)
If you encounter ALSA-related build errors:
```bash
sudo apt-get update
sudo apt-get install -y libasound2-dev libpkgconf-dev
```

### Build Failures
1. Check platform-specific workflow logs
2. Verify system dependencies are installed
3. Check cache key generation
4. Validate target architecture support

### Release Issues
1. Ensure all platform builds passed
2. Check artifact download logs
3. Validate package.json modifications
4. Verify npm authentication

## Migration Notes

This modular structure replaces the original monolithic `ci.yml` workflow. Key changes:

1. **Platform Separation**: Each platform has its own workflow
2. **Release Independence**: Release is now a separate workflow
3. **Better Debugging**: Isolated failures are easier to diagnose
4. **Parallel Execution**: Platforms can run in parallel
5. **Maintenance**: Easier to update individual components

## Future Enhancements

- [ ] Add actual reusable workflow calls (GitHub Actions limitation)
- [ ] Integration tests across platforms
- [ ] Automated binary size reporting
- [ ] Performance benchmarking
- [ ] Multi-architecture testing
