import { describe, expect, it } from "bun:test";

const {
  AudioDecoder,
  AudioPlayer,
  AudioRecorder,
  LoopedDecoder,
  PlaybackState,
  getSupportedFormats,
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
});
