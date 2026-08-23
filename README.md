# 🎵 MiniAudio Node

[![npm version](https://badge.fury.io/js/miniaudio_node.svg)](https://www.npmjs.com/package/miniaudio_node)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-blue.svg)](docs/PLATFORM_SUPPORT.md)

High-performance native audio for Node.js and Bun. MiniAudio Node exposes a Rust and Rodio audio engine through a typed N-API module.

This README is the documentation index. Detailed guides live in [`docs/`](docs/).

## Start here

| Goal | Guide |
| --- | --- |
| Install the package and play your first file | [Getting started](docs/GETTING_STARTED.md) |
| Learn the public API | [API guide](docs/API.md) |
| Check native binaries and operating-system requirements | [Platform support](docs/PLATFORM_SUPPORT.md) |
| Understand the Rust/N-API architecture | [Architecture](docs/ARCHITECTURE.md) |
| Build, test, and contribute locally | [Development guide](docs/DEVELOPMENT.md) |
| Understand test layers and hardware-dependent checks | [Testing guide](docs/TESTING.md) |
| Create and verify a release | [Publishing guide](docs/PUBLISH.md) |

Project reference:

- [Project structure](docs/PROJECT_STRUCTURE.md)
- [Changelog](docs/CHANGELOG.md)
- [Generated TypeScript declarations](index.d.ts)
- [Examples](examples/)

## Installation

```bash
bun add miniaudio_node
# or
npm install miniaudio_node
```

Published packages include prebuilt native binaries for the six targets listed in the [platform support guide](docs/PLATFORM_SUPPORT.md). Rust is only required when building the repository from source.

## Quick example

```typescript
import { AudioPlayer } from "miniaudio_node";

const player = new AudioPlayer();
player.loadFile("./audio/music.mp3");
player.setVolume(0.8);
player.play();

// Playback remains on the native audio thread.
setTimeout(() => player.pause(), 5_000);
```

`AudioPlayer` also supports buffer and Base64 loading, seeking, device enumeration, and playback-state queries. See the [API guide](docs/API.md) for the complete surface, including recording, passthrough, decoding, mixing, queues, converters, buffers, and noise generators.

## Supported formats

The default decoder reports support for WAV, MP3, FLAC, OGG/Vorbis, AAC, and M4A. Opus is not enabled by the default Rodio decoder features and is therefore not reported as supported.

## Development

```bash
bun install
bun run build
cargo test --all-targets
bun test
bun run tsc --noEmit
```

See the [development guide](docs/DEVELOPMENT.md) for the full command reference and the [testing guide](docs/TESTING.md) for hardware-aware test behavior.

## License

MiniAudio Node is released under the [MIT License](LICENSE).
