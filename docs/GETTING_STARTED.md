# Getting started

MiniAudio Node provides native audio playback for Node.js and Bun. The npm package includes prebuilt binaries, so most users do not need a Rust toolchain.

## Requirements

- Node.js 18 or newer, or Bun 1.0 or newer.
- An audio output device for playback.
- A published target supported by the package. See [Platform support](PLATFORM_SUPPORT.md).

Rust and platform build tools are only required when building the repository from source. See [Development](DEVELOPMENT.md) for that workflow.

## Install

```bash
bun add miniaudio_node
# or
npm install miniaudio_node
```

## Play a file

```typescript
import { AudioPlayer } from "miniaudio_node";

const player = new AudioPlayer();
player.loadFile("./audio/music.mp3");
player.setVolume(0.8);
player.play();

console.log(player.getState());       // "Playing"
console.log(player.getDuration());    // seconds
console.log(player.getCurrentTime()); // seconds
```

`play()` starts playback or resumes a paused player. `pause()` preserves the current position. `stop()` stops playback and resets the player to the stopped state.

```typescript
player.pause();
player.play();       // resume
player.seekTo(30);   // seek to 30 seconds; preserves the paused/playing state
player.stop();
```

Positions are expressed in seconds. `seekTo()` validates the position, clamps it to the loaded track where appropriate, and preserves whether playback was paused or playing. Seeking a loaded track does not start playback.

## Load a buffer or Base64 audio

The buffer APIs accept the file bytes as a JavaScript number array. They are useful when audio comes from `fetch`, a file input, or another data source.

```typescript
import { AudioPlayer } from "miniaudio_node";

const response = await fetch("https://example.com/audio.mp3");
const bytes = Array.from(new Uint8Array(await response.arrayBuffer()));

const player = new AudioPlayer();
player.loadBuffer(bytes);
player.play();
```

For Base64 input:

```typescript
const player = new AudioPlayer();
player.loadBase64(base64Audio);
player.play();
```

The input must contain a decodable audio file, not raw PCM samples. Invalid, empty, or unsupported data raises a JavaScript error.

## Use the convenience helpers

```typescript
import {
  createAudioPlayer,
  getAudioMetadata,
  quickPlay,
} from "miniaudio_node";

const player = createAudioPlayer({ volume: 0.6, autoPlay: false });
player.loadFile("./audio/voice.wav");
player.play();

const quickPlayer = quickPlay("./audio/notification.ogg", {
  volume: 0.7,
  autoPlay: true,
});

const metadata = getAudioMetadata("./audio/voice.wav");
console.log(metadata.duration);
```

`AudioMetadata.duration` is populated from the decoder. Title, artist, and album tag extraction is not currently provided and may be `undefined`.

## Inspect devices and formats

```typescript
import {
  getInputDevices,
  getSupportedFormats,
  isFormatSupported,
} from "miniaudio_node";

console.log(getSupportedFormats());
console.log(isFormatSupported("mp3"));
console.log(getInputDevices());
```

`AudioPlayer.getDevices()` lists output devices and may return an empty array when no output device is available. Input devices are available through `getInputDevices()` and `AudioRecorder`/`AudioPassthrough` helpers.

## Handle errors

Native validation errors are thrown synchronously by the public methods. Check file paths and input data before loading, and keep volume values between `0.0` and `1.0`.

```typescript
try {
  const player = new AudioPlayer();
  player.loadFile("./audio/missing.mp3");
  player.play();
} catch (error) {
  console.error("Audio operation failed:", error);
}
```

If a package binary cannot be loaded, first confirm that the runtime and operating system are in the [published support matrix](PLATFORM_SUPPORT.md). For source builds, consult the [development guide](DEVELOPMENT.md).

## Continue learning

- [API guide](API.md) — classes, functions, options, and data-flow components.
- [Platform support](PLATFORM_SUPPORT.md) — published binaries and native requirements.
- [Examples](../examples/) — runnable JavaScript and TypeScript examples.
