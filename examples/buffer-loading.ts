/**
 * TypeScript Example: Loading audio from buffers and base64 data
 *
 * This example demonstrates how to load audio data from:
 * 1. Raw buffer data (number[])
 * 2. Base64 encoded audio data
 */

// Import the native module and types using Bun's ESM import syntax
import { AudioPlayer, PlaybackState, AudioPlayerConfig } from "../index.js";

// Example 1: Load audio from a buffer
function loadFromBuffer(): void {
  console.log("=== Loading audio from buffer ===");

  const player: AudioPlayer = new AudioPlayer();

  // Create a minimal valid WAV file buffer (44 bytes header + 4 bytes data)
  // This is a very short silent audio clip
  const wavHeader: number[] = [
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

  try {
    player.loadBuffer(wavHeader);
    console.log("✅ Buffer loaded successfully");
    console.log(
      "State:",
      player.getState() === PlaybackState.Loaded ? "Loaded" : "Not loaded",
    );
    console.log("Current file:", player.getCurrentFile());

    // You can now play the audio
    // player.play();
  } catch (error: any) {
    console.error("❌ Failed to load buffer:", error.message);
  }
}

// Example 2: Load audio from base64 data
function loadFromBase64(): void {
  console.log("\n=== Loading audio from base64 ===");

  const player: AudioPlayer = new AudioPlayer();

  // Base64 encoded minimal WAV file (same as above)
  const base64Wav: string =
    "UklGRiQAAABXQVZFZm10IBAAAAABAAEAQB8AAEAfAAABAAgAZGF0YQQAAAAA";

  try {
    player.loadBase64(base64Wav);
    console.log("✅ Base64 audio loaded successfully");
    console.log(
      "State:",
      player.getState() === PlaybackState.Loaded ? "Loaded" : "Not loaded",
    );
    console.log("Current file:", player.getCurrentFile());

    // You can now play the audio
    // player.play();
  } catch (error: any) {
    console.error("❌ Failed to load base64:", error.message);
  }
}

// Example 3: Create player with configuration and load buffer
function loadWithConfig(): void {
  console.log("\n=== Loading with configuration ===");

  const config: AudioPlayerConfig = {
    volume: 0.5,
  };

  // Note: createAudioPlayer doesn't support buffer loading directly,
  // so we create the player and then load the buffer
  const player: AudioPlayer = new AudioPlayer();
  player.setVolume(config.volume || 1.0);

  const wavHeader: number[] = [
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

  try {
    player.loadBuffer(wavHeader);
    console.log("✅ Buffer loaded with volume:", player.getVolume());
    console.log(
      "State:",
      player.getState() === PlaybackState.Loaded ? "Loaded" : "Not loaded",
    );
  } catch (error: any) {
    console.error("❌ Failed to load buffer with config:", error.message);
  }
}

// Example 4: Error handling with types
function demonstrateTypeSafeErrors(): void {
  console.log("\n=== Type-safe error handling ===");

  const player: AudioPlayer = new AudioPlayer();

  // Try to load empty buffer
  try {
    player.loadBuffer([]);
    console.log("❌ Should have thrown error for empty buffer");
  } catch (error: any) {
    console.log("✅ Correctly caught empty buffer error:", error.message);
  }

  // Try to load empty base64
  try {
    player.loadBase64("");
    console.log("❌ Should have thrown error for empty base64");
  } catch (error: any) {
    console.log("✅ Correctly caught empty base64 error:", error.message);
  }

  // Try to load invalid base64
  try {
    player.loadBase64("invalid-base64!");
    console.log("❌ Should have thrown error for invalid base64");
  } catch (error: any) {
    console.log("✅ Correctly caught invalid base64 error:", error.message);
  }
}

// Run all examples
function runAllExamples(): void {
  console.log("🎵 TypeScript Audio Buffer Loading Examples\n");

  try {
    loadFromBuffer();
    loadFromBase64();
    loadWithConfig();
    demonstrateTypeSafeErrors();

    console.log("\n✅ All TypeScript examples completed successfully!");
  } catch (error: any) {
    console.error("❌ TypeScript example failed:", error);
  }
}

// Run examples if this file is executed directly
if (import.meta.main) {
  runAllExamples();
}

export {
  loadFromBuffer,
  loadFromBase64,
  loadWithConfig,
  demonstrateTypeSafeErrors,
};
