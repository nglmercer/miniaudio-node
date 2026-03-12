
import { describe, it, expect, beforeEach, afterEach } from "bun:test";
const {
  AudioPlayer,
  PlaybackState,
} = await import("../../index.js");

import {
  safeInitializeAudio,
  isAudioSystemAvailable,
} from "../utils/test-helpers.js";

describe("AudioPlayer Extended Buffer Loading", () => {
  let player: any = null;

  beforeEach(async () => {
    safeInitializeAudio();
    if (isAudioSystemAvailable()) {
      player = new AudioPlayer();
    }
  });

  afterEach(() => {
    try {
      if (player && player.isPlaying && player.isPlaying()) {
        player.stop();
      }
    } catch (error) {
      // Ignore cleanup errors
    }
  });

  it("should correctly extract duration when loading a buffer", () => {
    if (!isAudioSystemAvailable()) {
      console.warn("Skipping test: Audio system not available");
      return;
    }
    // Create a minimal valid WAV file buffer (44 bytes header + 4 bytes data)
    // 44100 Hz, 16-bit mono, 4 bytes of data = 2 samples = 2/44100 seconds
    const wavHeader = [
      0x52, 0x49, 0x46, 0x46, // "RIFF"
      0x24, 0x00, 0x00, 0x00, // File size - 8
      0x57, 0x41, 0x56, 0x45, // "WAVE"
      0x66, 0x6d, 0x74, 0x20, // "fmt "
      0x10, 0x00, 0x00, 0x00, // Format chunk size
      0x01, 0x00,             // Audio format (PCM)
      0x01, 0x00,             // Number of channels
      0x44, 0xac, 0x00, 0x00, // Sample rate (44100)
      0x88, 0x58, 0x01, 0x00, // Byte rate
      0x02, 0x00,             // Block align
      0x10, 0x00,             // Bits per sample
      0x64, 0x61, 0x74, 0x61, // "data"
      0x04, 0x00, 0x00, 0x00, // Data chunk size
      0x00, 0x00, 0x00, 0x00  // 4 bytes of silence
    ];

    player.loadBuffer(wavHeader);
    expect(player.getState()).toBe(PlaybackState.Loaded);

    const duration = player.getDuration();
    expect(duration).toBeGreaterThan(0);
    // 4 bytes of data / (2 bytes per sample * 1 channel) = 2 samples
    // 2 samples / 44100 Hz = 0.00004535 seconds
    expect(duration).toBeCloseTo(2 / 44100, 6);
  });

  it("should provide descriptive error when loading corrupted buffer", () => {
    if (!isAudioSystemAvailable()) {
      console.warn("Skipping test: Audio system not available");
      return;
    }
    const corruptedBuffer = [0x52, 0x49, 0x46, 0x46, 0x00, 0x00, 0x00, 0x00];

    try {
      player.loadBuffer(corruptedBuffer);
      expect(true).toBe(false); // Should not reach here
    } catch (error: any) {
      expect(error.message).toContain("Failed to decode audio buffer");
      expect(error.message).toContain("The format may be unsupported or the data may be corrupted");
    }
  });
});
