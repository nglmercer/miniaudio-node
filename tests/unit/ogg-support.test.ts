
import { describe, it, expect, beforeEach, afterEach } from "bun:test";
const {
  AudioPlayer,
  PlaybackState,
} = await import("../../index.js");

import {
  safeInitializeAudio,
  isAudioSystemAvailable,
} from "../utils/test-helpers.js";

describe("AudioPlayer OGG Support", () => {
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

  it("should attempt to decode OGG header without 'Unrecognized format' error", () => {
    if (!isAudioSystemAvailable()) {
      console.warn("Skipping test: Audio system not available");
      return;
    }

    // Minimal OGG header (OggS)
    const oggBuffer = [
      0x4f, 0x67, 0x67, 0x53, // OggS
      0x00,                   // Stream structure version
      0x02,                   // Header type (BOS)
      0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Granule position
      0x01, 0x00, 0x00, 0x00, // Serial number
      0x00, 0x00, 0x00, 0x00, // Page sequence number
      0x00, 0x00, 0x00, 0x00, // Checksum
      0x01,                   // Number of segments
      0x00                    // Segment table
    ];

    try {
      player.loadBuffer(oggBuffer);
    } catch (error: any) {
      // If it fails with "corrupted" or "io error", it means it TRIED to decode it as OGG
      // If it fails with "Unrecognized format", then our fix didn't work.
      expect(error.message).not.toContain("Unrecognized format");
    }
  });
});
