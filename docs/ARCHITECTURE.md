# Architecture

MiniAudio Node is a native N-API module. JavaScript and TypeScript call generated bindings, while Rust owns audio devices, decoding, timing, buffering, and native worker state.

## Runtime layers

```text
JavaScript / TypeScript
        │
        ▼
Generated N-API bindings (index.cjs, index.mjs, index.d.ts, index.d.mts)
        │
        ▼
Rust public modules and exported classes
        │
        ├── Rodio decoding and output streams
        └── CPAL input/output device access
```

The package root contains the generated loader and the platform-specific `.node` binaries. `src/lib.rs` wires the Rust modules together and re-exports their N-API classes and functions at the package boundary.

## Rust modules

| Module | Responsibility |
| --- | --- |
| `player.rs` | Thread-safe file, buffer, and Base64 playback; volume, state, seeking, and monotonic time tracking. |
| `decoder.rs` | File/data decoding, duration inspection, bounded slices, and configured sample-rate/channel conversion. |
| `stream.rs` | Explicitly opened output streams and `AudioStreamBuilder` settings. |
| `audio_passthrough.rs` | Input-to-output loopback, independent device configuration, conversion, latency buffering, and level callbacks. |
| `input.rs` | Recording input streams, callbacks, levels, and bounded latest-sample retention. |
| `buffer.rs` | Owned PCM sample buffers and asynchronous buffer playback. |
| `conversions.rs` | Sample-rate, channel-count, and sample-type conversion for interleaved data. |
| `mixer.rs` | Source management, per-source volume/pan/enabled state, frame sampling, and native real-time mixing. |
| `queue.rs` | Queue state, monotonic source IDs, current-index maintenance, and connected producer/consumer wrappers. |
| `noise/` | White, pink, blue, violet, brownian, velvet, and triangular noise generators. |
| `math.rs` | Decibel and linear-gain conversion helpers. |
| `types.rs` | N-API enums, objects, and public configuration types. |
| `utils.rs` | Audio initialization, format reporting, device information, test tones, and debug logging. |

## Audio data conventions

Decoded and generated PCM data is represented as interleaved signed 16-bit samples at the JavaScript API boundary. A two-channel frame is stored as:

```text
[left0, right0, left1, right1, ...]
```

Converters operate on complete frames. The sample-rate converter computes a fractional source position for each destination frame and interpolates each channel independently. Channel conversion maps source frames to target frames rather than treating the interleaved vector as one mono stream.

## Device and stream ownership

Rodio output streams must remain alive for the lifetime of their sinks. The player, stream, passthrough, and mixer therefore retain their native output stream handles alongside the playback state. Input and output device configurations are negotiated separately; passthrough conversion bridges their sample-rate and channel differences. The realtime mixer reads immutable source snapshots and atomic per-source controls, so JavaScript-side source updates do not require mutex acquisition on the audio callback path.

Long-running playback and recording state is held behind synchronization primitives or native atomics. JavaScript methods control that state without keeping a JavaScript event-loop callback active for the duration of playback. `SamplesBuffer.play()` and `testTone()` start native playback and return promptly.

## Queue ownership

`AudioSourceQueue` owns an `Arc`-backed queue state. `SourcesQueueInput.fromQueue(queue)` and `SourcesQueueOutput.fromQueue(queue)` clone a reference to that same state, allowing producer and consumer wrappers to communicate. Calling their constructors separately creates independent queues by design.

## Build boundary

`@napi-rs/cli` compiles the Rust crate and generates the package loader and TypeScript declarations. The release workflow builds the six targets in [Platform support](PLATFORM_SUPPORT.md), then packages the generated loader, declarations, README, license, and native artifacts.

For repository setup and commands, see [Development](DEVELOPMENT.md). For test boundaries and hardware-dependent behavior, see [Testing](TESTING.md).
