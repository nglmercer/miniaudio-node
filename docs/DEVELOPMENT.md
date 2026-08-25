# Development guide

This guide covers local work on the Rust N-API module and the generated package files.

## Prerequisites

- Bun 1.0 or newer.
- Node.js 18 or newer.
- Rust stable and Cargo.
- A platform C/C++ toolchain supported by Rust.
- Linux: ALSA and pkg-config development packages.

On Debian or Ubuntu:

```bash
sudo apt-get update
sudo apt-get install -y libasound2-dev libpkgconf-dev
```

See [Platform support](PLATFORM_SUPPORT.md) for operating-system-specific requirements.

## Set up the repository

```bash
git clone https://github.com/nglmercer/miniaudio-node.git
cd miniaudio-node
bun install --frozen-lockfile
```

## Build the native module

Build for the current host:

```bash
bun run build
```

For an unoptimized development build:

```bash
bun run build:debug
```

The N-API CLI generates the native `.node` artifact, `index.cjs`/`index.mjs` loaders, and `index.d.ts`/`index.d.mts` declarations. The repository does not require a custom loader patch step after the build; use the CLI's target and platform arguments directly. Both bindings must be generated: `--js`/`--dts` are CLI-only flags with no `.napirc.json` equivalent, so a bare `napi build --platform` falls back to emitting `index.js`.

To build a particular target locally:

```bash
bunx napi build --platform --release --target x86_64-unknown-linux-gnu --js index.cjs --dts index.d.ts
bunx napi build --platform --release --target x86_64-unknown-linux-gnu --esm --js index.mjs --dts index.d.mts
```

Only build targets listed in [Platform support](PLATFORM_SUPPORT.md) are published by the release workflow.

## Verification commands

Run the focused checks while developing:

```bash
# Rust formatting and linting
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings

# Rust behavior tests
cargo test --all-targets

# Bun API and integration tests
bun test

# TypeScript declarations and examples
bun run tsc --noEmit
```

The combined project scripts are:

| Command | Purpose |
| --- | --- |
| `bun run build` | Release build for the current host. |
| `bun run build:debug` | Debug build for the current host. |
| `bun run build:watch` | Rebuild when Rust sources change. |
| `bun run test` | Run Bun tests. |
| `bun run tsc --noEmit` | Type-check the repository without emitting files. |
| `bun run lint` | Run Rust formatting and Clippy checks. |
| `bun run format` | Format Rust sources. |
| `bun run clean` | Remove N-API and Cargo build artifacts. |
| `bun run examples:<name>` | Run one of the configured examples. |

## Source layout

The Rust code is split by responsibility instead of placing the complete implementation in `src/lib.rs`. See [Architecture](ARCHITECTURE.md) and [Project structure](PROJECT_STRUCTURE.md) before adding a new module.

Generated files should normally be refreshed by `bun run build` rather than edited manually. The package entry points are `index.cjs` (CommonJS) and `index.mjs` (ESM), selected through the `exports` map; the generated public declarations are in `index.d.ts` and `index.d.mts`.

## Adding a feature

1. Identify the owning Rust module and keep the N-API boundary small.
2. Add deterministic Rust tests for arithmetic, state, conversion, queue, or retention behavior.
3. Add Bun tests for public validation and integration behavior.
4. Update the generated declarations by rebuilding.
5. Update the relevant article under `docs/` and add an example when the API is user-facing.
6. Run formatting, Clippy, Rust tests, Bun tests, and TypeScript checks.

Hardware-dependent tests should clearly identify their device requirement. See [Testing](TESTING.md) for the expected distinction between deterministic checks and audio-hardware checks.

## Pull requests

Keep pull requests focused, include the verification commands you ran, and document platform-specific limitations. Changes to the public API should update `index.d.ts` and `index.d.mts` through generation and should be reflected in [API](API.md) and the [changelog](CHANGELOG.md).
