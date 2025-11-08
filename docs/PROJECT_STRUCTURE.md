# MiniAudio Node - Project Structure

This document explains the professional project structure and organization of MiniAudio Node.

## 📁 Directory Structure

```
miniaudio-node/
├── 🦀 native/                      # Rust native module
│   ├── src/
│   │   └── lib.rs                 # Rust FFI implementation
│   ├── Cargo.toml                 # Rust dependencies
│   ├── index.js                   # Native module entry point
│   ├── package.json               # Native package configuration
│   └── target/                    # Rust build artifacts
│
├── 🧪 tests/                       # Test suite
│   ├── unit/                      # Unit tests
│   │   └── audio-player.test.ts   # AudioPlayer tests
│   └── integration/               # Integration tests
│       └── playback.test.ts       # Core API integration tests
│
├── 📚 examples/                    # Example usage
│   ├── usage.js                   # Basic JavaScript example
│   └── typescript/               # TypeScript examples
│       └── advanced.ts           # Advanced features with types
│
├── 🔧 scripts/                     # Build and utility scripts
│   ├── build.ts                   # Main build script
│   ├── clean.ts                   # Cleanup script
│   ├── dev.ts                     # Development server
│   ├── install.js                 # Post-install script
│   └── simple-build.js            # Simple build script
│
├── ⚙️ config/                      # Configuration files
│   ├── tsconfig.json              # TypeScript configuration
│   ├── eslint.config.js           # ESLint configuration
│   ├── prettier.config.js         # Prettier configuration
│   └── bunfig.toml                # Bun configuration
│
├── 📖 docs/                        # Documentation
│   ├── CHANGELOG.md               # Version history
│   ├── LICENSE                    # License file
│   └── PROJECT_STRUCTURE.md       # This file
│
├── 🏗️ benchmarks/                  # Performance benchmarks (placeholder)
│
├── 📄 package.json                 # Package configuration
├── 📝 justfile                     # Just command runner
├── 🚫 .gitignore                   # Git ignore rules
└── 📖 README.md                    # Main documentation
```

## 🎯 Key Design Principles

### 1. **Separation of Concerns**
- **Source Code**: TypeScript implementation in `src/`
- **Native Code**: Rust implementation isolated in `native/`
- **Tests**: Separate `unit/` and `integration/` test directories
- **Configuration**: All config files in `config/`
- **Documentation**: Complete documentation in `docs/`

### 2. **Scalability**
- Modular TypeScript structure with feature-based organization
- Separate type definitions for better maintainability
- Configurable build system supporting multiple environments
- Extensive test coverage with both unit and integration tests

### 3. **Developer Experience**
- Hot-reload development server with `bun run dev`
- Comprehensive CLI commands via `justfile`
- Automated code quality checks (ESLint, Prettier, TypeScript)
- Rich examples covering basic to advanced usage

### 4. **Cross-Platform Support**
- Native module compiled for multiple platforms
- Platform-specific tests in `integration/cross-platform.test.ts`
- CI/CD pipeline testing on Windows, macOS, and Linux
- Conditional native binary loading based on platform

## 🔧 Build System Architecture

### TypeScript Compilation
```bash
# Development build with watch mode
bun run dev

# Production build
bun run build

# Type checking only
bun run typecheck
```

### Native Module Compilation
```bash
# Release build (optimized)
bun run build:native

# Debug build
bun run build:native:debug

# Cross-platform compilation
# Handled by GitHub Actions in CI/CD
```

### Testing Pipeline
```bash
# All tests
bun test

# Unit tests only
bun run test:unit

# Integration tests only
bun run test:integration

# Coverage report
bun run test:coverage
```

## 📦 Package Management

### Bun as Primary Package Manager
- Uses `bun.lockb` for fast, reliable dependency locking
- Bun's native TypeScript compilation
- Optimized for performance and developer experience

### Development Dependencies
- **ESLint + Prettier**: Code quality and formatting
- **TypeScript**: Type safety and compilation
- **VitePress**: Documentation generation
- **Changesets**: Semantic versioning and changelog generation

### Runtime Dependencies
- **Zero runtime dependencies** for the final package
- All audio processing handled by the native Rust module
- Minimal bundle size for optimal performance

## 🚀 Development Workflow

### 1. **Initial Setup**
```bash
git clone https://github.com/audio-dev/miniaudio-node.git
cd miniaudio-node
just setup  # Install dependencies and configure environment
```

### 2. **Daily Development**
```bash
just dev          # Start development server with hot reload
just test         # Run all tests
just lint         # Check code quality
```

### 3. **Building for Release**
```bash
just clean        # Clean all build artifacts
just build        # Build production version
just test-all     # Run complete test suite
```

### 4. **Release Process**
```bash
just version-bump patch  # Bump version
just release             # Publish to npm
```

## 🧪 Testing Strategy

### Unit Tests (`tests/unit/`)
- Test individual functions and classes in isolation
- Mock external dependencies
- Fast execution, suitable for CI/CD
- Focus on business logic and type safety

### Integration Tests (`tests/integration/`)
- Test real audio playback functionality
- Cross-platform compatibility
- Performance under load
- Native module integration

### Fixtures (`tests/fixtures/`)
- Small audio files for testing
- Multiple formats supported
- Platform-independent test data

## 📚 Documentation Strategy

### API Documentation (`docs/api/`)
- Auto-generated from TypeScript types
- Code examples for each method
- Parameter descriptions and return types
- Error handling documentation

### User Guides (`docs/guides/`)
- Getting started tutorials
- Advanced usage patterns
- Troubleshooting guides
- Best practices

### Examples (`examples/`)
- Real-world usage scenarios
- Progressive complexity
- Both JavaScript and TypeScript
- Self-contained, runnable code

## 🔒 Security Considerations

### Code Security
- Rust memory safety prevents buffer overflows
- TypeScript type safety prevents runtime errors
- Automated security scanning in CI/CD
- Dependency vulnerability scanning

### File System Access
- Validated file paths before audio loading
- Sandboxed native module execution
- No arbitrary code execution
- Resource usage monitoring

## 📊 Performance Optimization

### Build Optimization
- Tree shaking for minimal bundle size
- Native module compiled with optimizations
- Lazy loading of native binaries
- Efficient TypeScript compilation

### Runtime Optimization
- Direct Rust-to-JS interface with minimal overhead
- Memory-efficient audio processing
- Asynchronous operations where possible
- Resource cleanup and garbage collection

## 🔮 Future Extensibility

### Plugin System
The modular structure supports future plugin development:
- Audio effects plugins
- Format decoder plugins
- Device driver plugins
- Visualization plugins

### API Evolution
Backward-compatible API evolution through:
- Semantic versioning with changesets
- TypeScript interface versioning
- Migration guides for breaking changes
- Deprecation warnings and timelines

This professional structure ensures maintainability, scalability, and excellent developer experience while maintaining high performance and cross-platform compatibility.
