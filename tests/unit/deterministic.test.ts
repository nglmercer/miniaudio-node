import { describe, expect, it } from "bun:test";
import { mkdtempSync, rmSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const {
  AudioDecoder,
  AudioPlayer,
  AudioRecorder,
  AudioSourceQueue,
  AudioStream,
  AudioStreamBuilder,
  ChannelCountConverter,
  Mixer,
  MixerSource,
  LoopedDecoder,
  PlaybackState,
  SampleRateConverter,
  quickPlay,
  SampleTypeConverter,
  SamplesBuffer,
  StaticSamplesBuffer,
  createAudioPlayer,
  getSupportedFormats,
  isDebugEnabled,
  setDebug,
} = await import("../../index.js");

function makeSilenceWav(
  durationMs: number,
  sampleRate = 8_000,
  channels = 1,
): number[] {
  const frameCount = Math.floor((sampleRate * durationMs) / 1000);
  const blockAlign = channels * 2;
  const dataSize = frameCount * blockAlign;
  const bytes = new Uint8Array(44 + dataSize);
  const view = new DataView(bytes.buffer);
  const writeAscii = (offset: number, value: string) => {
    for (let index = 0; index < value.length; index++) {
      bytes[offset + index] = value.charCodeAt(index);
    }
  };

  writeAscii(0, "RIFF");
  view.setUint32(4, 36 + dataSize, true);
  writeAscii(8, "WAVE");
  writeAscii(12, "fmt ");
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);
  view.setUint16(22, channels, true);
  view.setUint32(24, sampleRate, true);
  view.setUint32(28, sampleRate * blockAlign, true);
  view.setUint16(32, blockAlign, true);
  view.setUint16(34, 16, true);
  writeAscii(36, "data");
  view.setUint32(40, dataSize, true);
  return Array.from(bytes);
}

describe("deterministic native validation", () => {
  it("loads without an audio device and reports the enabled formats", () => {
    const player = new AudioPlayer();
    expect(player.getState()).toBe(PlaybackState.Stopped);
    expect(player.getVolume()).toBe(1);
    expect(getSupportedFormats()).toEqual(["wav", "mp3", "flac", "ogg"]);
  });

  it("keeps loaded and paused seeks from starting playback", () => {
    const player = new AudioPlayer();
    player.loadBuffer(makeSilenceWav(1_000));

    player.seekTo(0.25);
    expect(player.isPlaying()).toBe(false);
    expect(player.getState()).toBe(PlaybackState.Loaded);
    expect(player.getCurrentTime()).toBeCloseTo(0.25, 2);
    player.seekTo(0);
    expect(player.getCurrentTime()).toBeCloseTo(0, 2);
  });

  it("decodes only the requested slice and rejects unbounded loops", () => {
    const decoder = AudioDecoder.fromData(makeSilenceWav(1_000));
    const slice = decoder.decodeSlice(0.25, 0.5);
    expect(slice.length).toBeGreaterThanOrEqual(1_900);
    expect(slice.length).toBeLessThanOrEqual(2_100);

    const looped = new LoopedDecoder(decoder, 1_000_000_000);
    expect(() => looped.decodeLooped()).toThrow(/safety limit|capacity/);
  });

  it("rejects malformed recorder device IDs", () => {
    const recorder = new AudioRecorder();
    expect(() => recorder.start("garbage")).toThrow(/Invalid device ID/);
    expect(recorder.isRecording()).toBe(false);
  });

  it("keeps recorder ring-buffer reads non-destructive and validates capacity", () => {
    const recorder = new AudioRecorder();
    expect(recorder.getRingBufferSamples()).toEqual([]);
    expect(() => recorder.setRingBufferSize(0)).toThrow(/greater than zero/);
    recorder.setRingBufferSize(4);
    expect(recorder.getRingBufferSamples()).toEqual([]);
    recorder.clear();
    expect(recorder.getRingBufferSamples()).toEqual([]);
    expect(recorder.getLevels()).toEqual({ peak: 0, rms: 0 });
  });

  it("rejects invalid sample-buffer metadata instead of creating invalid durations", () => {
    expect(() => new SamplesBuffer(0, 44_100, [])).toThrow(/greater than zero/);
    expect(() => new SamplesBuffer(2, 0, [])).toThrow(/greater than zero/);
    expect(() => new SamplesBuffer(70_000, 44_100, [])).toThrow(/must not exceed/);
    expect(() => new SamplesBuffer(2, 44_100, [1])).toThrow(/complete frame/);
    expect(() => SamplesBuffer.fromBytes([1, 2, 3], 1, 44_100)).toThrow(
      /complete 16-bit samples/,
    );
    expect(() => new StaticSamplesBuffer(2, 44_100, [1])).toThrow(/complete frame/);

    const buffer = SamplesBuffer.fromBytes([0x34, 0x12], 1, 44_100);
    expect(buffer.getSamples()).toEqual([0x1234]);
    expect(Number.isFinite(buffer.getDuration())).toBe(true);
  });

  it("rejects unsupported sample-type bit depths and handles identity explicitly", () => {
    expect(() => new SampleTypeConverter(12, 16)).toThrow(/8, 16, 24, or 32/);
    expect(() => new SampleTypeConverter(16, 20)).toThrow(/8, 16, 24, or 32/);

    const identity = new SampleTypeConverter(16, 16);
    expect(identity.convert([-123, 456])).toEqual([-123, 456]);
  });

  it("mixes deterministic source controls without opening a device", () => {
    const mixer = Mixer.withConfig(44_100, 2, 2);
    const source = new MixerSource("tone", [1_000, -1_000], 44_100, 2);
    mixer.addSource(source);
    expect(mixer.sampleAt(0)).toEqual([1_000, -1_000]);

    source.setVolume(0.5);
    source.setPan(1);
    mixer.setMasterVolume(0.5);
    expect(mixer.sampleAt(0)).toEqual([0, -250]);
    source.setEnabled(false);
    expect(mixer.sampleAt(0)).toEqual([0, 0]);
  });

  it("covers deterministic queues, converters, and stream-builder validation", () => {
    const channels = new ChannelCountConverter(1, 2);
    expect(channels.convert([100, 200])).toEqual([100, 100, 200, 200]);

    const rates = new SampleRateConverter(48_000, 44_100, 2);
    expect(
      rates.convert([0, 10_000, 1_000, 11_000, 2_000, 12_000, 3_000, 13_000]).length,
    ).toBe(6);

    const queue = new AudioSourceQueue();
    const first = queue.addBuffer([1, 2], "first");
    const second = queue.addBuffer([3, 4], "second");
    expect([first, second]).toEqual(["source_0", "source_1"]);
    expect(queue.getLength()).toBe(2);
    queue.removeSource(first);
    expect(queue.getSource(second).buffer).toEqual([3, 4]);

    const builder = new AudioStreamBuilder();
    builder.setSampleRate(0);
    expect(() => builder.build()).toThrow(/greater than zero/);
    builder.setSampleRate(44_100);
    builder.setChannels(2);
    builder.setBufferSize(512);
    const stream = builder.build();
    expect(stream.getState()).toBe(PlaybackState.Stopped);
    stream.stop();
    expect(AudioStream.getSupportedConfigs().length).toBeGreaterThan(0);
  });

  it("applies debug configuration on both player factory paths", () => {
    setDebug(false);
    createAudioPlayer({ debug: true });
    expect(isDebugEnabled()).toBe(true);

    setDebug(true);
    expect(() => {
      // Loading fails deterministically, but the config is applied before the
      // load attempt and must not be silently ignored.
      const path = "miniaudio-node-missing-file.wav";
      quickPlay(path, { debug: false });
    }).toThrow(/File not found/);
    expect(isDebugEnabled()).toBe(false);
    setDebug(false);
  });

  it("preserves decoder metadata and propagates file failures from looped clones", () => {
    const directory = mkdtempSync(join(tmpdir(), "miniaudio-node-"));
    const filePath = join(directory, "source.wav");
    try {
      writeFileSync(filePath, Buffer.from(makeSilenceWav(100)));
      const decoder = new AudioDecoder(filePath);
      const looped = new LoopedDecoder(decoder, 2);
      const clone = looped.getDecoder();
      expect(clone.getSampleRate()).toBe(decoder.getSampleRate());
      expect(clone.getChannels()).toBe(decoder.getChannels());

      unlinkSync(filePath);
      expect(() => clone.decodeToSamples()).toThrow(/open|File|No such file/i);
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  });
});
