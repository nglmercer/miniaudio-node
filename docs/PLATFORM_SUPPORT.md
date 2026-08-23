# Platform support

Each published npm package contains native binaries for the following six targets:

| Operating system | Architecture | Rust target | Published artifact |
| --- | --- | --- | --- |
| Windows | x64 | `x86_64-pc-windows-msvc` | `miniaudio_node.win32-x64-msvc.node` |
| Windows | ia32 | `i686-pc-windows-msvc` | `miniaudio_node.win32-ia32-msvc.node` |
| macOS | x64 | `x86_64-apple-darwin` | `miniaudio_node.darwin-x64.node` |
| macOS | arm64 | `aarch64-apple-darwin` | `miniaudio_node.darwin-arm64.node` |
| Linux | x64, glibc | `x86_64-unknown-linux-gnu` | `miniaudio_node.linux-x64-gnu.node` |
| Linux | arm64, glibc | `aarch64-unknown-linux-gnu` | `miniaudio_node.linux-arm64-gnu.node` |

The package supports Node.js 18 or newer and Bun 1.0 or newer on those targets.

## Linux compatibility

The published Linux binaries are built for glibc and use the ALSA development/runtime stack. Alpine Linux and other musl-based distributions are not currently published targets. A generated N-API loader can contain generic branches for musl, Android, FreeBSD, or other architectures; those branches do not mean that this package ships a matching binary.

For a musl-based or otherwise unsupported environment, build the native module locally and validate it against that environment. The repository does not currently promise a prebuilt artifact for it.

## Native prerequisites for source builds

Prebuilt package users do not need these tools. They are required for `bun run build` from the repository:

- Rust stable and Cargo.
- Bun and Node.js.
- Windows: the MSVC build tools from Visual Studio or Build Tools.
- macOS: Xcode Command Line Tools.
- Debian/Ubuntu Linux: ALSA and pkg-config development packages.

For Debian/Ubuntu:

```bash
sudo apt-get update
sudo apt-get install -y libasound2-dev libpkgconf-dev
```

## Device support

Audio availability depends on the host operating system and its devices. Output, input, and passthrough APIs can fail when a container, CI runner, permission policy, or headless session has no usable audio device. The package reports those native errors; it does not emulate hardware.

Use these APIs to inspect the host:

```typescript
import {
  getAvailableHosts,
  getInputDevices,
  supportedOutputConfigs,
} from "miniaudio_node";

console.log(getAvailableHosts());
console.log(getInputDevices());
console.log(supportedOutputConfigs());
```

## Checking the selected binary

The generated `index.js` chooses a native `.node` file from the package according to the current platform and architecture. If module loading fails:

1. Confirm `process.platform` and `process.arch` match one of the rows above.
2. Confirm the installed package contains the corresponding `.node` artifact.
3. On Linux, confirm the host uses a compatible glibc version and has the required audio runtime libraries.
4. If the target is unsupported, use a local source build or move to a published target.

See [Development](DEVELOPMENT.md) for local builds and [Publishing](PUBLISH.md) for the release matrix.
