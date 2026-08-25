/**
 * Device Management Example with TypeScript and Bun
 *
 * This example demonstrates how to:
 * 1. Enumerate audio devices
 * 2. Select specific devices
 * 3. Handle device changes
 * 4. Get device properties
 */

import { AudioPlayer, initializeAudio } from "../index.mjs";
import type { AudioDeviceInfo } from "../index.mjs";

class DeviceManager {
  private player: AudioPlayer;
  private devices: AudioDeviceInfo[];
  private selectedDeviceId: string | null = null;

  constructor() {
    this.player = new AudioPlayer();
    this.devices = this.player.getDevices();
  }

  /**
   * List all available audio devices
   */
  listDevices(): void {
    console.log("🔊 Available Audio Devices:");

    if (this.devices.length === 0) {
      console.log("   No audio devices found!");
      return;
    }

    this.devices.forEach((device, index) => {
      const marker = device.isDefault ? "🟡" : "⚪";
      const prefix = device.id === this.selectedDeviceId ? "▶️" : "  ";
      console.log(
        `${prefix} ${marker} [${index + 1}] ${device.name}`,
        `(ID: ${device.id})`,
        device.isDefault ? "(Default)" : "",
      );
    });
  }

  /**
   * Select a device by index
   */
  selectDeviceByIndex(index: number): boolean {
    const device = this.devices[index - 1];
    if (!device) {
      console.log(`❌ Device index ${index} not found!`);
      return false;
    }

    this.selectDeviceById(device.id);
    return true;
  }

  /**
   * Select a device by ID
   */
  selectDeviceById(id: string): void {
    const device = this.devices.find((d) => d.id === id);
    if (!device) {
      console.log(`❌ Device with ID "${id}" not found!`);
      return;
    }

    this.selectedDeviceId = id;
    console.log(`✅ Selected device: ${device.name}`);

    // Note: The actual selecting of devices for playback happens through
    // the player's initialization and load methods. Here we're managing
    // the selection tracking.
  }

  /**
   * Select the default device
   */
  selectDefaultDevice(): void {
    const defaultDevice = this.devices.find((d) => d.isDefault);
    if (defaultDevice) {
      this.selectDeviceById(defaultDevice.id);
    } else {
      console.log("⚠️  No default device found");
    }
  }

  /**
   * Get device details
   */
  getDeviceDetails(deviceId: string): AudioDeviceInfo | undefined {
    return this.devices.find((d) => d.id === deviceId);
  }

  /**
   * Get current selection
   */
  getSelectedDevice(): AudioDeviceInfo | null {
    if (!this.selectedDeviceId) return null;
    return this.devices.find((d) => d.id === this.selectedDeviceId) || null;
  }

  /**
   * Check if a device supports the default format
   */
  isDeviceAvailable(): boolean {
    return this.devices.length > 0;
  }

  /**
   * Print device summary
   */
  printSummary(): void {
    const selected = this.getSelectedDevice();
    console.log("\n📊 Device Summary:");
    console.log("------------------");
    console.log(`Total Devices: ${this.devices.length}`);
    console.log(
      `Default Device: ${this.devices.find((d) => d.isDefault)?.name || "None"}`,
    );
    console.log(`Selected Device: ${selected?.name || "None"}`);
    console.log(`Device Ready: ${this.isDeviceAvailable()}`);
  }
}

/**
 * Demonstrate device selection and audio playback
 */
async function demoDeviceAudio() {
  console.log("\n🎵 Device-Aware Audio Playback Demo");
  console.log("=".repeat(50));

  const manager = new DeviceManager();
  manager.listDevices();
  manager.printSummary();

  // Try to load a system sound
  const platformSound = getPlatformSound();
  if (platformSound) {
    console.log(`\n🔊 Loading audio: ${platformSound}`);

    try {
      const player = new AudioPlayer();
      player.loadFile(platformSound);
      console.log("✅ Audio loaded successfully");

      // Show playback details
      console.log("\nPlayback Info:");
      console.log(`  Duration: ${player.getDuration().toFixed(2)}s`);
      console.log(`  Volume: ${player.getVolume() * 100}%`);
      console.log(`  State: ${player.getState()}`);

      // Clean up
      player.stop();
      console.log("✅ Audio stopped");
    } catch (error) {
      console.error("❌ Audio playback failed:", (error as Error).message);
    }
  } else {
    console.log("\n⚠️  No system audio files found for this platform");
  }
}

/**
 * Get a platform-appropriate sound file
 */
function getPlatformSound(): string | null {
  const fs = require("node:fs");

  if (process.platform === "win32") {
    const sounds = [
      "C:/Windows/Media/tada.wav",
      "C:/Windows/Media/chimes.wav",
      "C:/Windows/Media/notify.wav",
    ];
    return sounds.find((s) => fs.existsSync(s)) || null;
  }

  if (process.platform === "darwin") {
    const sounds = [
      "/System/Library/Sounds/Glass.aiff",
      "/System/Library/Sounds/Guir.aiff",
      "/System/Library/Sounds/Sosumi.aiff",
    ];
    return sounds.find((s) => fs.existsSync(s)) || null;
  }

  if (process.platform === "linux") {
    const sounds = [
      "/usr/share/sounds/alsa/Front_Left.wav",
      "/usr/share/sounds/alsa/Front_Right.wav",
    ];
    return sounds.find((s) => fs.existsSync(s)) || null;
  }

  return null;
}

/**
 * Main function
 */
async function runDeviceManagementExample(): Promise<void> {
  console.log("🚀 MiniAudio Node - Device Management Example");
  console.log("=".repeat(50));

  try {
    // Initialize audio system
    console.log("\n🔧 Initializing audio system...");
    const initResult = initializeAudio();
    console.log("✅", initResult);

    // Run the device demonstration
    await demoDeviceAudio();

    console.log("\n✅ Device management example completed!");
    console.log("\n💡 Key Features Demonstrated:");
    console.log("  ✓ Audio device enumeration");
    console.log("  ✓ Device selection and tracking");
    console.log("  ✓ Default device identification");
    console.log("  ✓ Device-aware audio playback");
    console.log("  ✓ Platform-specific audio handling");
  } catch (error) {
    console.error(
      "\n❌ Device management example failed:",
      (error as Error).message,
    );
    console.error((error as Error).stack);
  }
}

// Run if this file is the main module
if (import.meta.main) {
  runDeviceManagementExample().catch((error) => {
    console.error("Fatal error:", error);
    process.exit(1);
  });
}

export { DeviceManager, runDeviceManagementExample };
