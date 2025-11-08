# MiniAudio Node Justfile
# Simplified commands for development workflow
# Install just: https://github.com/casey/just

# Default command
default:
    @just --list

# Development
dev:
    bun scripts/dev.ts

dev-verbose:
    bun scripts/dev.ts --verbose

# Build commands
build:
    bun scripts/build.ts

build-debug:
    bun scripts/build.ts --debug

build-verbose:
    bun scripts/build.ts --verbose

# Testing
test:
    bun test

test-watch:
    bun test --watch

test-coverage:
    bun test --coverage

test-unit:
    bun test tests/unit/

test-integration:
    bun test tests/integration/

# Code quality
lint:
    bun eslint 'src/**/*.ts' --config config/eslint.config.js

lint-fix:
    bun eslint 'src/**/*.ts' --config config/eslint.config.js --fix

format:
    bun prettier --write 'src/**/*.ts' --config config/prettier.config.js

format-check:
    bun prettier --check 'src/**/*.ts' --config config/prettier.config.js

typecheck:
    bun tsc --noEmit --project config/tsconfig.json

# Cleanup
clean:
    bun scripts/clean.ts

clean-verbose:
    bun scripts/clean.ts --verbose

clean-aggressive:
    bun scripts/clean.ts --aggressive

clean-dry:
    bun scripts/clean.ts --dry-run

# Examples
examples:
    just examples-basic && just examples-typescript

examples-basic:
    bun examples/javascript/basic.js

examples-typescript:
    bun examples/typescript/advanced.ts

# Documentation
docs-dev:
    bun run docs:dev

docs-build:
    bun run docs:build

docs-preview:
    bun run docs:preview

# Release workflow
prepare-release:
    just clean && just build

release:
    just prepare-release && bun run release

# Native module commands
native-build:
    cd native && cargo build --release

native-build-debug:
    cd native && cargo build

native-test:
    cd native && cargo test

native-clean:
    cd native && cargo clean

# Performance
bench:
    bun benchmarks/

bench-compare:
    bun benchmarks/compare.ts

# Analysis
analyze:
    just build-release && bun run analyze:size

# Install dependencies
install:
    bun install

install-dev:
    bun install --dev

# CI/CD helpers
ci-test:
    just lint && just format-check && just typecheck && just test

ci-build:
    just clean && just build && just test-integration

# Development helpers
status:
    @echo "MiniAudio Node Development Status"
    @echo "==============================="
    @echo "Rust: $(rustc --version)"
    @echo "Bun: $(bun --version)"
    @echo "Node: $(node --version 2>/dev/null || echo 'Not installed')"
    @echo "Git: $(git --version 2>/dev/null || echo 'Not installed')"
    @echo ""
    @echo "Available commands: just --list"

# Quick start for new developers
setup:
    @echo "🚀 Setting up MiniAudio Node development environment..."
    just install
    @echo "✅ Dependencies installed"
    @echo ""
    @echo "📝 Next steps:"
    @echo "  just dev        # Start development server"
    @echo "  just test       # Run tests"
    @echo "  just build      # Build project"
    @echo "  just examples   # Run examples"
    @echo ""
    @echo "📚 For more commands: just --list"

# Help
help:
    @echo "MiniAudio Node - Just Commands"
    @echo "=============================="
    @echo ""
    @echo "Development:"
    @echo "  just dev              Start development server"
    @echo "  just dev-verbose      Verbose development mode"
    @echo ""
    @echo "Building:"
    @echo "  just build            Build for production"
    @echo "  just build-debug      Build with debug symbols"
    @echo ""
    @echo "Testing:"
    @echo "  just test             Run all tests"
    @echo "  just test-watch       Watch mode testing"
    @echo "  just test-coverage    Generate coverage report"
    @echo ""
    @echo "Code Quality:"
    @echo "  just lint             Run linter"
    @echo "  just format           Format code"
    @echo "  just typecheck        Type checking"
    @echo ""
    @echo "Cleanup:"
    @echo "  just clean            Remove build artifacts"
    @echo "  just clean-aggressive  Remove everything including node_modules"
    @echo ""
    @echo "Examples:"
    @echo "  just examples         Run all examples"
    @echo "  just examples-basic   Run basic JavaScript examples"
    @echo "  just examples-typescript  Run advanced TypeScript examples"
    @echo ""
    @echo "For complete command list: just --list"

# Native development shortcuts
rust-check:
    cd native && cargo check

rust-clippy:
    cd native && cargo clippy -- -D warnings

rust-docs:
    cd native && cargo doc --open

# Git helpers
git-status:
    @git status --short && echo ""

git-log:
    @git log --oneline -10

git-diff:
    @git diff --stat

# Quick test with different configurations
test-all:
    just test-unit && just test-integration && just test-coverage

test-quick:
    bun test --quiet --timeout=5000

# Environment checks
check-deps:
    @echo "Checking dependencies..."
    @which rustc > /dev/null || (echo "❌ Rust not found" && exit 1)
    @which bun > /dev/null || (echo "❌ Bun not found" && exit 1)
    @echo "✅ All dependencies found"

check-project:
    @echo "Checking project structure..."
    @test -f "src/index.ts" || (echo "❌ src/index.ts not found" && exit 1)
    @test -f "native/src/lib.rs" || (echo "❌ native/src/lib.rs not found" && exit 1)
    @test -f "package.json" || (echo "❌ package.json not found" && exit 1)
    @echo "✅ Project structure is valid"

# Version management
version:
    @echo "Current version: $(cat package.json | jq -r .version)"

version-bump patch:
    @bun changeset bump
    @echo "📦 Version bumped to patch level"

version-bump minor:
    @bun changeset pre minor
    @echo "📦 Version bumped to minor level"

version-bump major:
    @bun changeset pre major
    @echo "📦 Version bumped to major level"
