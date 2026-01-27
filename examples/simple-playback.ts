/**
 * Simple Audio Playback Example with TypeScript and Bun
 *
 * This example demonstrates basic audio playback functionality
 * including device enumeration and simple audio file playback.
 */

// Import the native module and types
import { AudioPlayer, initializeAudio, getSupportedFormats } from "../index.js";

// Import苎 kinds of audio file paths for different platforms
function getPlatformSoundPaths(): string[] {
  const platform = process.platform;
  const filePath = process.argv[2] || null;
  if (filePath) return [filePath,filePath];
  if (platform === "win32") {
    return [
      "C:/Windows/Media/tada.wav",
      "C:/Windows/Media/chimes.wav",
      "C:/Windows/Media/notify.wav",
    ];
  } else if (platform === "darwin") {
    return [
      "/System/Library/Sounds/Glass.aiff",
      "/System/Library/Sounds/Guir.aiff",
      "/System/Library/Sounds/Sosumi.aiff",
    ];
  } else if (platform === "linux") {
    return [
      "/usr/share/sounds/alsa/Front_Left.wav",
      "/usr/share/sounds/alsa/Front_Right.wav",
    ];
  }

  return [];
}

async function runSimplePlayback() {
  console.log("🎵 MiniAudio Node - Simple Playback Example");
  console.log("=".repeat(50));

  try {
    // Initialize the audio system
    console.log("\n🔧 Initializing audio system...");
    const initResult = initializeAudio();
    console.log("✅", initResult);

    // Create audio player
    console.log("\n🎧 Creating audio player...");
    const player = new AudioPlayer();

    // Get available audio devices
    console.log("\n🔊 Available audio devices:");
    const devices = player.getDevices();
    devices.forEach((device, index) => {
      console.log(
        `  ${index + 1}. ${device.name}${device.isDefault ? " (Default)" : ""}`,
      );
    });

    // Get supported audio formats
    console.log("\n📋 Supported audio formats:");
    const formats = getSupportedFormats();
    console.log(`  ${formats.join(", ")}`);

    // Try to load and play a system sound
    const soundPaths = getPlatformSoundPaths();
    const systemSoundPath = soundPaths.find((path) => {
      const fs = require("node:fs");
      return fs.existsSync(path);
    });

    if (systemSoundPath) {
      console.log(`\n🎵 Loading audio file: ${systemSoundPath}`);

      try {
        // Load the audio file
        player.loadFile(systemSoundPath);

        console.log("\n✅ File loaded successfully!");
        console.log("   Initial volume:", player.getVolume());
        console.log("   Is playing:", player.isPlaying());
        console.log("   Playback state:", player.getState());

        // Start playback
        console.log("\n▶️  Starting playback...");
        player.play();

        // Monitor playback for 5 seconds
        const monitorInterval = setInterval(() => {
          if (player.isPlaying()) {
            console.log(
              `   Current time: ${player.getCurrentTime().toFixed(1)}s / ${player.getDuration().toFixed(1)}s`,
            );
          }
        }, 500);

        // Test volume changes and controls
        setTimeout(() => {
          console.log("\n🔊 Adjusting volume to 50%...");
          player.setVolume(0.5);
        }, 2000);

        setTimeout(() => {
          console.log("\n⏸️  Pausing playback...");
          player.pause();
          console.log("   Is playing:", player.isPlaying());
        }, 3500);

        setTimeout(() => {
          console.log("\n▶️  Resuming playback...");
          player.play();
          console.log("   Is playing:", player.isPlaying());
        }, 5000);

        setTimeout(() => {
          console.log("\n🔊 Setting volume to 100%...");
          player.setVolume(1.0);
        }, 6500);

        setTimeout(() => {
          console.log("\n⏹️  Stopping playback...");
          player.stop();
          console.log("   Is playing:", player.isPlaying());
          console.log("   Final volume:", player.getVolume());

          clearInterval(monitorInterval);
          console.log("\n✅ Test completed successfully!");
        }, 8000);
      } catch (error) {
        console.error(
          "\n❌ Failed to load audio file:",
          (error as Error).message,
        );
      }
    } else {
      console.log("no audio files found");
    }

    // Check if a file path was provided as argument
    if (process.argv[2]) {
      const filePath = process.argv[2];
      console.log(`\n🔊 Loading user-provided file: ${filePath}`);

      try {
        player.loadFile(filePath);
        console.log("✅ File loaded!");
        console.log("   Duration:", player.getDuration(), "seconds");

        player.play();
        console.log("▶️  Playing file...");

        // Play until finished
        setTimeout(() => {
          if (player.isPlaying()) {
            console.log("   Still playing...");
          }
        }, 1000);

        // Wait for duration to complete
        const totalDuration = player.getDuration() * 1000;
        setTimeout(
          () => {
            player.stop();
            console.log("\n✅ Playback finished!");
            process.exit(0);
          },
          Math.min(totalDuration + 500, 15000),
        ); // Limit to 15 seconds max
      } catch (error) {
        console.error(
          "\n❌ Failed to load user-provided file:",
          (error as Error).message,
        );
      }
    }
  } catch (error) {
    console.error("\n❌ Error during playback:", (error as Error).message);
    console.error((error as Error).stack);
  }
}

// Run the example
if (import.meta.main) {
  runSimplePlayback().catch((error) => {
    console.error("Fatal error:", error);
    process.exit(1);
  });
}

export { runSimplePlayback };
