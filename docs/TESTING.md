# Testing guide

The test suite is layered so deterministic behavior can run without physical audio hardware, while device-dependent paths are exercised only when a usable audio system is present.

## Test layers

### Rust tests

The Rust suite covers behavior that does not require opening a real device, including:

- playback-clock and seek arithmetic;
- sample-rate, channel-count, and sample-type conversion;
- queue IDs and current-index maintenance;
- recorder retention and bounded latest-sample behavior;
- mixer state and frame calculations;
- format, buffer, noise, and utility behavior.

Run it with:

```bash
cargo test --all-targets
```

### Bun tests

Bun tests exercise the generated JavaScript API, validation, file and buffer loading, device information, and integration paths:

```bash
bun test
```

The hardware-oriented suites check audio-system availability first. When the host has no usable audio system, those cases are reported as skipped rather than counted as hardware coverage. The always-run `tests/unit/deterministic.test.ts` suite covers loading, validation, decoding, state, and allocation guards without a device, so headless CI still executes meaningful tests.

Run a focused file when iterating:

```bash
bun test tests/unit/audio-player.test.ts
bun test tests/integration/playback.test.ts
```

### TypeScript

The generated declarations, examples, and TypeScript tests are checked explicitly:

```bash
bun run tsc --noEmit
```

TypeScript failures must make the command fail; the CI workflows do not convert them into warnings.

## Local quality gate

Before opening a pull request, run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
bun test
bun run tsc --noEmit
```

Use `bun run lint` for the formatting and Clippy checks together.

## Hardware-aware testing

For playback, recording, and passthrough changes, test on a host with:

- a usable output device;
- a usable input device when recording or loopback is involved;
- permissions for the runtime to access those devices;
- the same operating-system family and architecture as the target being changed.

The repository's helper utilities discover common system sound files and safely initialize the audio system. They are intended to prevent false failures in headless environments, not to replace dedicated device testing.

When adding a hardware-dependent test:

1. Make the hardware requirement explicit in the test name or description.
2. Skip only when the required audio capability is unavailable.
3. Keep validation and state assertions outside the hardware-only branch whenever possible.
4. Add deterministic Rust coverage for calculations and state transitions.

## CI and release checks

The regular CI matrix builds all six published native targets and runs a Node.js smoke test plus the deterministic suite on host-native artifacts where the runner can execute them. It then runs the hardware-oriented suites, which may skip device cases on headless runners. Cross-compiled Linux arm64 and Windows ia32 artifacts are build-checked but are not executed on incompatible runners.

The release workflow first runs format, Clippy, Rust tests, and TypeScript quality gates. It then builds the same six targets and runs Node.js plus deterministic and hardware-oriented Bun tests on the configured host-native release artifacts before packaging them. See [Publishing](PUBLISH.md) for the release sequence and [Platform support](PLATFORM_SUPPORT.md) for the exact matrix.

## Coverage expectations

The suite is designed to catch deterministic regressions and public API contract errors. It is not a substitute for real-device testing, codec compatibility testing across every operating system, or long-duration soak tests. Document any new platform or hardware assumptions when changing those areas.
