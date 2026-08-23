# Project structure

MiniAudio Node keeps the JavaScript package boundary small and groups native audio behavior by responsibility.

## Repository tree

```text
miniaudio-node/
├── .github/workflows/
│   ├── ci.yml                 # Build and test the published target matrix
│   ├── lint.yml               # Format, Clippy, and TypeScript checks
│   └── release.yml            # Quality-gated build and publish workflow
├── docs/
│   ├── API.md                 # Public API guide
│   ├── ARCHITECTURE.md        # Runtime and Rust module design
│   ├── CHANGELOG.md           # Version history
│   ├── DEVELOPMENT.md         # Local setup and commands
│   ├── GETTING_STARTED.md     # Installation and first playback
│   ├── LICENSE                # Documentation copy of the MIT license
│   ├── PLATFORM_SUPPORT.md    # Native target and runtime matrix
│   ├── PROJECT_STRUCTURE.md   # This document
│   ├── PUBLISH.md             # Release and npm publishing guide
│   └── TESTING.md             # Deterministic and hardware-aware tests
├── examples/                  # JavaScript and TypeScript usage examples
├── scripts/                   # Build and artifact helper scripts
├── src/
│   ├── audio_passthrough.rs   # Input-to-output loopback
│   ├── buffer.rs              # Owned PCM buffers
│   ├── conversions.rs         # Sample, rate, and channel conversion
│   ├── decoder.rs             # File/data decoding and conversion
│   ├── input.rs               # Recording and input-device handling
│   ├── lib.rs                 # Module wiring and flat N-API exports
│   ├── math.rs                # Gain conversion helpers
│   ├── mixer.rs               # Source mixing and real-time output
│   ├── noise/                 # Noise generator implementations
│   ├── player.rs              # High-level playback
│   ├── queue.rs               # Source queues and producer/consumer views
│   ├── stream.rs              # Configurable output streams
│   ├── types.rs               # Public N-API types and enums
│   └── utils.rs               # Initialization, formats, devices, and tones
├── tests/
│   ├── debug/                 # Audio debugging utilities
│   ├── integration/           # Public API integration tests
│   ├── unit/                  # AudioPlayer API tests
│   └── utils/                 # Platform and audio-system helpers
├── Cargo.toml                 # Rust crate and native dependencies
├── Cargo.lock                 # Locked Rust dependency versions
├── index.d.ts                 # Generated TypeScript declarations
├── index.js                   # Generated N-API platform loader
├── LICENSE                    # Package license
├── package.json               # npm metadata and scripts
└── README.md                  # Documentation index
```

## Package boundary

`src/lib.rs` re-exports the public Rust modules so N-API generates a flat JavaScript API. The build produces:

- `index.js`, which selects the native binary for the current platform and architecture;
- `index.d.ts`, which describes the generated public API;
- a platform-specific `.node` binary.

The npm package includes those generated files, the license, and the README. The published native matrix is documented in [Platform support](PLATFORM_SUPPORT.md).

## Where to make changes

Use the owning module for native behavior:

- playback state, timing, seeking, and volume → `src/player.rs`;
- decoded data and conversion options → `src/decoder.rs` and `src/conversions.rs`;
- input capture and retained samples → `src/input.rs`;
- device loopback → `src/audio_passthrough.rs`;
- output configuration → `src/stream.rs`;
- queues and mixer state → `src/queue.rs` and `src/mixer.rs`;
- exported enums and object types → `src/types.rs`;
- device, format, and utility functions → `src/utils.rs`.

Add behavior tests next to the relevant Rust module when the behavior is deterministic. Add public API and integration tests under `tests/`. Hardware-dependent tests should use the audio-availability helpers and be explicit about skip conditions; see [Testing](TESTING.md).

## Generated files

Do not hand-edit `index.js` or `index.d.ts` for normal API changes. Add or update the Rust N-API definitions and run:

```bash
bun run build
```

The N-API CLI is responsible for generating the loader and declarations. No separate loader patch step is part of the supported build workflow.

## Documentation map

- [Getting started](GETTING_STARTED.md) for users installing the package.
- [API](API.md) for exported classes and functions.
- [Architecture](ARCHITECTURE.md) for native data flow and ownership.
- [Development](DEVELOPMENT.md) for local builds and contribution workflow.
- [Testing](TESTING.md) for test layers and CI expectations.
- [Publishing](PUBLISH.md) for release automation.
