/**
 * Unit Tests for AudioPlayer
 *
 * These tests verify core functionality of AudioPlayer class
 * using Bun's built-in test runner with cross-platform compatibility.
 */

import { describe, it, expect, beforeEach, afterEach } from "bun:test";
const {
  AudioPlayer,
  initializeAudio,
  getSupportedFormats,
  createAudioPlayer,
  quickPlay,
  isFormatSupported,
  getAudioMetadata,
  PlaybackState,
  setDebug,
} = await import("../../index.js");

// Import types separately
import type { AudioDeviceInfo, AudioPlayerConfig } from "../../index.js";
import {
  safeInitializeAudio,
  isAudioSystemAvailable,
  PLATFORM,
} from "../utils/test-helpers.js";
setDebug(false);

describe("AudioPlayer", () => {
  let player: typeof AudioPlayer | any = null;

  beforeEach(async () => {
    // Initialize audio system before each test with error handling
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

  describe("Constructor", () => {
    it("should create a new AudioPlayer instance", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      expect(player).toBeInstanceOf(AudioPlayer);
    });

    it("should have default volume of 1.0", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      expect(player.getVolume()).toBe(1.0);
    });

    it("should not be playing initially", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      expect(player.isPlaying()).toBe(false);
    });
  });

  describe("Volume Control", () => {
    it("should set volume correctly", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      player.setVolume(0.5);
      expect(player.getVolume()).toBeCloseTo(0.5);
    });

    it("should accept minimum volume", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      player.setVolume(0.0);
      expect(player.getVolume()).toBe(0.0);
    });

    it("should accept maximum volume", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      player.setVolume(1.0);
      expect(player.getVolume()).toBe(1.0);
    });

    it("should throw error for volume below 0.0", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      expect(() => player.setVolume(-0.1)).toThrow(
        "Volume must be between 0.0 and 1.0",
      );
    });

    it("should throw error for volume above 1.0", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      expect(() => player.setVolume(1.1)).toThrow(
        "Volume must be between 0.0 and 1.0",
      );
    });
  });

  describe("Device Management", () => {
    it("should return available devices", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      const devices = player.getDevices();
      expect(Array.isArray(devices)).toBe(true);
      expect(devices.length).toBeGreaterThan(0);
    });

    it("should return device objects with required properties", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      const devices = player.getDevices();
      devices.forEach((device: any) => {
        expect(device).toHaveProperty("id");
        expect(device).toHaveProperty("name");
        expect(device).toHaveProperty("isDefault");
        expect(typeof device.id).toBe("string");
        expect(typeof device.name).toBe("string");
        expect(typeof device.isDefault).toBe("boolean");
      });
    });
  });

  describe("Playback State", () => {
    it("should report not playing when no file is loaded", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      expect(player.isPlaying()).toBe(false);
    });

    it("should report not playing when stopped", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      // This test may need adjustment based on actual implementation
      expect(player.isPlaying()).toBe(false);
    });
  });

  describe("File Loading", () => {
    it("should throw error when loading non-existent file", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      expect(() => player.loadFile("non-existent-file.mp3")).toThrow();
    });

    it("should throw error when loading with empty path", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      expect(() => player.loadFile("")).toThrow();
    });
  });

  describe("Buffer Loading", () => {
    it("should load valid audio buffer", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      // Create a minimal valid WAV file buffer (44 bytes header + 4 bytes data)
      const wavHeader = [
        0x52,
        0x49,
        0x46,
        0x46, // "RIFF"
        0x24,
        0x00,
        0x00,
        0x00, // File size - 8
        0x57,
        0x41,
        0x56,
        0x45, // "WAVE"
        0x66,
        0x6d,
        0x74,
        0x20, // "fmt "
        0x10,
        0x00,
        0x00,
        0x00, // Format chunk size
        0x01,
        0x00, // Audio format (PCM)
        0x01,
        0x00, // Number of channels
        0x44,
        0xac,
        0x00,
        0x00, // Sample rate (44100)
        0x88,
        0x58,
        0x01,
        0x00, // Byte rate
        0x02,
        0x00, // Block align
        0x10,
        0x00, // Bits per sample
        0x64,
        0x61,
        0x74,
        0x61, // "data"
        0x04,
        0x00,
        0x00,
        0x00, // Data chunk size
        0x00,
        0x00,
        0x00,
        0x00, // 4 bytes of silence
      ];

      expect(() => player.loadBuffer(wavHeader)).not.toThrow();
      expect(player.getState()).toBe(PlaybackState.Loaded);
    });

    it("should throw error for empty buffer", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      const emptyBuffer: number[] = [];
      expect(() => player.loadBuffer(emptyBuffer)).toThrow(
        "Audio buffer is empty",
      );
    });

    it("should throw error for invalid audio buffer", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      const invalidBuffer = [1, 2, 3, 4, 5];
      expect(() => player.loadBuffer(invalidBuffer)).toThrow();
    });

    it("should update current file to buffer identifier", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      // Create a minimal valid WAV file buffer
      const wavHeader = [
        0x52,
        0x49,
        0x46,
        0x46, // "RIFF"
        0x24,
        0x00,
        0x00,
        0x00, // File size - 8
        0x57,
        0x41,
        0x56,
        0x45, // "WAVE"
        0x66,
        0x6d,
        0x74,
        0x20, // "fmt "
        0x10,
        0x00,
        0x00,
        0x00, // Format chunk size
        0x01,
        0x00, // Audio format (PCM)
        0x01,
        0x00, // Number of channels
        0x44,
        0xac,
        0x00,
        0x00, // Sample rate (44100)
        0x88,
        0x58,
        0x01,
        0x00, // Byte rate
        0x02,
        0x00, // Block align
        0x10,
        0x00, // Bits per sample
        0x64,
        0x61,
        0x74,
        0x61, // "data"
        0x04,
        0x00,
        0x00,
        0x00, // Data chunk size
        0x00,
        0x00,
        0x00,
        0x00, // 4 bytes of silence
      ];

      player.loadBuffer(wavHeader);
      const currentFile = player.getCurrentFile();
      expect(currentFile).toContain("__BUFFER__");
    });
  });

  describe("Base64 Loading", () => {
    it("should load valid base64 audio data", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      // Base64 encoded minimal WAV file (same as above)
      const base64Wav =
        "UklGRiQAAABXQVZFZm10IBAAAAABAAEAQB8AAEAfAAABAAgAZGF0YQQAAAAA";

      expect(() => player.loadBase64(base64Wav)).not.toThrow();
      expect(player.getState()).toBe(PlaybackState.Loaded);
    });

    it("should throw error for empty base64 string", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      expect(() => player.loadBase64("")).toThrow("Base64 data is empty");
    });

    it("should throw error for invalid base64 string", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      expect(() => player.loadBase64("invalid-base64!")).toThrow();
    });

    it("should throw error for base64 with invalid audio data", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      // Valid base64 but invalid audio data
      const invalidAudioBase64 = "SGVsbG8gV29ybGQ="; // "Hello World" in base64

      expect(() => player.loadBase64(invalidAudioBase64)).toThrow();
    });

    it("should update current file to buffer identifier", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      // Base64 encoded minimal WAV file
      const base64Wav =
        "UklGRiQAAABXQVZFZm10IBAAAAABAAEAQB8AAEAfAAABAAgAZGF0YQQAAAAA";

      player.loadBase64(base64Wav);
      const currentFile = player.getCurrentFile();
      expect(currentFile).toContain("__BUFFER__");
    });
  });

  describe("Metadata", () => {
    it("should return duration as number", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      const duration = player.getDuration();
      expect(typeof duration).toBe("number");
      expect(duration).toBeGreaterThanOrEqual(0);
    });

    it("should return current time as number", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      const currentTime = player.getCurrentTime();
      expect(typeof currentTime).toBe("number");
      expect(currentTime).toBeGreaterThanOrEqual(0);
    });
  });
});

describe("Audio System", () => {
  describe("initializeAudio", () => {
    it("should initialize audio system successfully", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      expect(() => initializeAudio()).not.toThrow();
      const result = initializeAudio();
      expect(typeof result).toBe("string");
      expect(result).toContain("initialized");
    });

    it("should handle multiple initializations gracefully", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      // Multiple initializations should not cause issues
      for (let i = 0; i < 3; i++) {
        const result = initializeAudio();
        expect(typeof result).toBe("string");
        expect(result).toContain("initialized");
      }
    });
  });

  describe("getSupportedFormats", () => {
    it("should return array of supported formats", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      const formats = getSupportedFormats();
      expect(Array.isArray(formats)).toBe(true);
      expect(formats.length).toBeGreaterThan(0);
    });

    it("should include common audio formats", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      const formats = getSupportedFormats();
      expect(formats).toContain("wav");
      expect(formats).toContain("mp3");
      expect(formats).toContain("flac");
      expect(formats).toContain("ogg");
    });

    it("should contain only lowercase format names", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      const formats = getSupportedFormats();
      formats.forEach((format: any) => {
        expect(format).toBe(format.toLowerCase());
      });
    });

    it("should return consistent results across calls", () => {
      if (!isAudioSystemAvailable()) {
        console.warn("Skipping test: Audio system not available");
        return;
      }

      const formats1 = getSupportedFormats();
      const formats2 = getSupportedFormats();
      expect(formats1).toEqual(formats2);
    });
  });
});

describe("Error Handling", () => {
  it("should handle playback operations without loaded file", () => {
    if (!isAudioSystemAvailable()) {
      console.warn("Skipping test: Audio system not available");
      return;
    }

    const player = new AudioPlayer();

    expect(() => player.play()).toThrow("Player not initialized");
    expect(() => player.pause()).toThrow("Player not initialized");
    expect(() => player.stop()).toThrow("Player not initialized");
  });

  it("should handle invalid volume values", () => {
    if (!isAudioSystemAvailable()) {
      console.warn("Skipping test: Audio system not available");
      return;
    }

    const player = new AudioPlayer();

    expect(() => player.setVolume(-0.1)).toThrow(
      "Volume must be between 0.0 and 1.0",
    );
    expect(() => player.setVolume(1.1)).toThrow(
      "Volume must be between 0.0 and 1.0",
    );
  });

  it("should handle invalid file paths gracefully", () => {
    if (!isAudioSystemAvailable()) {
      console.warn("Skipping test: Audio system not available");
      return;
    }

    const player = new AudioPlayer();

    expect(() => player.loadFile("")).toThrow();
    expect(() => player.loadFile(null as any)).toThrow();
    expect(() => player.loadFile(undefined as any)).toThrow();
  });
});

describe("Integration Tests", () => {
  it("should maintain state across multiple operations", () => {
    if (!isAudioSystemAvailable()) {
      console.warn("Skipping test: Audio system not available");
      return;
    }

    const player = new AudioPlayer();

    // Test volume persistence
    player.setVolume(0.7);
    expect(player.getVolume()).toBeCloseTo(0.7);

    // Test that volume doesn't reset after other operations
    const devices = player.getDevices();
    expect(devices.length).toBeGreaterThan(0);
    expect(player.getVolume()).toBeCloseTo(0.7);
  });

  it("should handle rapid state changes", () => {
    if (!isAudioSystemAvailable()) {
      console.warn("Skipping test: Audio system not available");
      return;
    }

    const player = new AudioPlayer();

    // Rapid volume changes
    for (let i = 0; i < 10; i++) {
      player.setVolume(i / 10);
      expect(player.getVolume()).toBeCloseTo(i / 10);
    }

    // Rapid device queries
    for (let i = 0; i < 10; i++) {
      const devices = player.getDevices();
      expect(Array.isArray(devices)).toBe(true);
    }
  });

  it("should work with helper functions", () => {
    if (!isAudioSystemAvailable()) {
      console.warn("Skipping test: Audio system not available");
      return;
    }

    // Test createAudioPlayer helper
    const player1 = createAudioPlayer({ volume: 0.5 });
    expect(player1).toBeInstanceOf(AudioPlayer);
    expect(player1.getVolume()).toBeCloseTo(0.5);

    // Test quickPlay helper with invalid file (should not crash)
    expect(() => {
      const player2 = quickPlay("non-existent.mp3", { autoPlay: false });
      expect(player2).toBeInstanceOf(AudioPlayer);
    }).toThrow();
  });

  it("should handle platform-specific behavior", () => {
    if (!isAudioSystemAvailable()) {
      console.warn("Skipping test: Audio system not available");
      return;
    }

    console.log(`Running on platform: ${PLATFORM.platform}`);

    const player = new AudioPlayer();
    const devices = player.getDevices();

    // Should work on all platforms
    expect(Array.isArray(devices)).toBe(true);
    expect(player.getVolume()).toBeGreaterThanOrEqual(0);
    expect(player.getVolume()).toBeLessThanOrEqual(1);
  });
});

describe("PlaybackState Enum", () => {
  it("should use enum values correctly", () => {
    if (!isAudioSystemAvailable()) {
      console.warn("Skipping test: Audio system not available");
      return;
    }

    const player = new AudioPlayer();
    expect(player.getState()).toBe(PlaybackState.Stopped);
  });
});
