# API guide

The generated [`index.d.ts`](../index.d.ts) is the authoritative reference for exact TypeScript signatures. This guide groups the API by use case and explains the behavior that is easiest to miss.

## Core playback

### `AudioPlayer`

`AudioPlayer` is the simplest interface for file and in-memory playback.

| Method | Description |
| --- | --- |
| `loadFile(path)` | Load a decodable audio file from disk. |
| `loadBuffer(bytes)` | Load encoded audio bytes supplied as `number[]`. |
| `loadBase64(value)` | Load an encoded audio file from a Base64 string. |
| `play()` | Start or resume playback. |
| `pause()` | Pause without losing the current position. |
| `stop()` | Stop playback and reset the current playback session. |
| `seekTo(seconds)` | Seek to a validated position in the loaded track. |
| `setVolume(value)` / `getVolume()` | Set or read a volume between `0.0` and `1.0`. |
| `getState()` | Return `Stopped`, `Loaded`, `Playing`, or `Paused`. |
| `getCurrentTime()` | Return the current position in seconds using a monotonic playback clock. |
| `getDuration()` | Return the loaded duration in seconds. |
| `getCurrentFile()` | Return the loaded file path, or a synthetic `__BUFFER__...` identifier for buffer input. |
| `getDevices()` | List available output devices; returns an empty array when enumeration finds none. |

Example:

```typescript
import { AudioPlayer, PlaybackState } from "miniaudio_node";

const player = new AudioPlayer();
player.loadFile("./audio/song.flac");
player.play();

if (player.getState() === PlaybackState.Playing) {
  console.log(`${player.getCurrentTime()} / ${player.getDuration()} seconds`);
}
```

`createAudioPlayer(config)` accepts `volume`, `autoPlay`, and `debug`. `quickPlay(path, config)` creates a configured player and can start it immediately.

### Supported formats and metadata

`getSupportedFormats()` and `isFormatSupported(format)` report WAV, MP3, FLAC, and OGG/Vorbis for the default decoder configuration. AAC, M4A, and Opus are not enabled by the default Rodio feature set.

`getAudioMetadata(path)` returns the decoded duration. `title`, `artist`, and `album` are optional and are not currently extracted from tags.

## Audio input and output

### `AudioRecorder`

```typescript
import { AudioRecorder } from "miniaudio_node";

const recorder = new AudioRecorder();
recorder.setRingBufferSize(44_100 * 10); // retain the latest 10 seconds
recorder.start();                         // use the default input device

console.log(recorder.getConfig());
console.log(recorder.getLevels());
const recentSamples = recorder.getRingBufferSamples();

recorder.stop();
```

`setRingBufferSize()` bounds the retained history in samples; when it overflows, the history keeps the newest samples. `getRingBufferSamples()` returns a non-destructive snapshot, so repeated reads do not consume the retained samples. `getBuffer()` returns the same bounded time window as a `SamplesBuffer`; `clear()` empties both retained stores and resets peak/RMS levels. `setOnData()` receives incoming sample chunks asynchronously through a bounded non-blocking queue; under sustained JavaScript backpressure, queued callback chunks may be dropped rather than blocking capture. `getLevels()` reports peak and RMS levels.

Pass a device ID returned by `getInputDevices()` to `start(deviceId)`. Malformed or unknown IDs are rejected; they never fall back to the first input device.

### `AudioPassthrough`

`AudioPassthrough` and `startPassthrough()` provide input-to-output loopback. Omit either device ID, or pass `null`, to select that system's default device.

```typescript
import { startPassthrough } from "miniaudio_node";

const passthrough = startPassthrough(undefined, undefined, 20);
console.log(passthrough.getSampleRate(), passthrough.getChannels());

// ...when finished
passthrough.stop();
```

Input and output streams negotiate their formats independently. The passthrough converts sample rate and channel layout when the devices do not expose identical configurations and normalizes supported CPAL integer/float input formats before transport. Use `AudioPassthrough.getInputDevices()` and `getOutputDevices()` to inspect devices across the available host backends; returned IDs include the host name.

### `AudioStream` and `AudioStreamBuilder`

Use the stream API when output configuration matters:

```typescript
import { AudioStreamBuilder } from "miniaudio_node";

const stream = new AudioStreamBuilder();
stream.setSampleRate(48_000);
stream.setChannels(2);
stream.setBufferSize(512);

const audio = stream.build();
audio.open();
audio.playFile("./audio/song.wav");
```

The builder stores the requested sample rate, channel count, and buffer size. `open()` applies those settings to the native output stream and reports an error if the device cannot provide them. `supportedOutputConfigs()` and `AudioStream.getSupportedConfigs()` expose common output configurations.

## Decoding and sample data

### `AudioDecoder` and `DecoderBuilder`

`AudioDecoder` can be created from a file or encoded byte array. It exposes sample rate, channel count, duration, reset, full decoding, bounded slice decoding, and mono/stereo checks.

```typescript
import { DecoderBuilder } from "miniaudio_node";

const builder = new DecoderBuilder();
builder.setSampleRate(48_000);
builder.setChannels(2);

const decoder = builder.buildFromFile("./audio/source.mp3");
const samples = decoder.decodeToSamples();
const preview = decoder.decodeSlice(0, 5);
```

The builder applies requested sample-rate and channel conversions while decoding. `setLoopEnabled()` and `setLoopCount()` configure `buildLooped()`. A finite loop count can be materialized with `decodeLooped()`; an infinite loop cannot be represented by a finite array and returns an error.

`decodeSlice()` skips and decodes only the requested time window. Materialized slices and finite loop results are capped at 100,000,000 interleaved samples; requests over that limit return an error before an oversized allocation is attempted.

### `SamplesBuffer`

`SamplesBuffer` stores signed 16-bit PCM samples with an explicit channel count and sample rate.

```typescript
import { SamplesBuffer } from "miniaudio_node";

const buffer = new SamplesBuffer(2, 44_100, samples);
console.log(buffer.getDuration());
buffer.play(); // returns after the output stream starts
```

`SamplesBuffer.fromBytes()` interprets bytes as little-endian 16-bit samples and rejects odd byte counts. Constructors reject zero rates or channels, channel counts above the native limit, and incomplete interleaved frames. `StaticSamplesBuffer` owns a buffer and exposes it through `getInner()`.

## Converters

All converter inputs use interleaved samples. A stereo frame is `[left, right]`; a three-channel frame contains three consecutive values.

| Class | Purpose |
| --- | --- |
| `SampleRateConverter` | Fractional-rate linear interpolation per channel, for example 48,000 Hz to 44,100 Hz. Pass the channel count when converting interleaved multichannel data. |
| `ChannelCountConverter` | Convert mono, stereo, and arbitrary channel layouts frame by frame. |
| `SampleTypeConverter` | Convert between supported 8-, 16-, 24-, and 32-bit sample representations; unsupported bit depths are rejected. |

`SampleRateConverter` rejects zero source/target rates or channel counts, and `ChannelCountConverter` rejects zero source/target channel counts. For multichannel data, construct `SampleRateConverter(sourceRate, targetRate, channels)` so interpolation never crosses channel boundaries.

## Mixing

`MixerSource` represents PCM samples with their source rate and channel count. It supports volume, stereo pan (`-1.0` to `1.0`), and enabled/disabled state.

```typescript
import { Mixer, MixerSource } from "miniaudio_node";

const mixer = Mixer.withConfig(44_100, 2, 16);
const source = new MixerSource("voice", samples, 44_100, 1);
source.setVolume(0.8);
source.setPan(-0.2);
mixer.addSource(source);

const frame = mixer.sampleAt(250); // mixed frame at 250 ms
mixer.startMixing();
// mixer.stopMixing() when finished
```

`sampleAt()` applies source volume, pan, enabled state, and master volume. `startMixing()` creates a native output stream; it requires at least one source and a valid configuration. `stopMixing()` stops and releases that stream.

## Queues

`AudioSourceQueue` stores file or buffer sources and uses monotonic IDs that remain unique after removals. The current index is adjusted when sources are removed.

```typescript
import {
  AudioSourceQueue,
  SourcesQueueInput,
  SourcesQueueOutput,
} from "miniaudio_node";

const queue = new AudioSourceQueue();
const input = SourcesQueueInput.fromQueue(queue);
const output = SourcesQueueOutput.fromQueue(queue);

input.pushFile("./audio/one.mp3");
input.pushFile("./audio/two.mp3");

while (output.hasNext()) {
  console.log(output.pop());
}
```

The `fromQueue()` factories are important: they connect producer and consumer wrappers to the same queue. Constructing each wrapper independently creates separate queue state.

## Noise and utility functions

The package exports white, pink, blue, violet, brownian, velvet, and triangular noise generators. Each generator provides `getSamples()`, `getNext()`, and `reset()` where applicable.

Other utilities include:

- `initializeAudio()` and `getAudioInfo()` for the current default output device, format, channel count, and sample rate.
- `getAvailableHosts()`, `getInputDevices()`, and `getInputDevicesByHost()` for input enumeration.
- `dbToLinear()` and `linearToDb()` for gain conversion.
- `testTone(frequency, durationMs)` for a non-blocking sine-wave test tone.
- `setDebug()` and `isDebugEnabled()` for native diagnostic logging.

## Related documentation

- [Getting started](GETTING_STARTED.md)
- [Platform support](PLATFORM_SUPPORT.md)
- [Architecture](ARCHITECTURE.md)
- [Testing](TESTING.md)
