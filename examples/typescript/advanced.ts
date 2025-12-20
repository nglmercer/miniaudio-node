/**
 * Advanced Audio Playback Example with TypeScript and Bun
 *
 * This example demonstrates advanced features of miniaudio_node
 * including error handling, async operations, and type safety.
 */

// Import native module directly
const {
  AudioPlayer,
  initializeAudio,
  getSupportedFormats,
  createAudioPlayer,
  quickPlay,
  isFormatSupported,
  getAudioMetadata,
  PlaybackState,
} = await import("../../index.js");

// Import types separately
import type { AudioDeviceInfo,AudioPlayerConfig } from "../../index.js";

/**
 * Playlist manager class for handling multiple audio files
 */
class PlaylistManager {
  private player: any;
  private tracks: string[] = [];
  private currentTrackIndex: number = 0;
  private isPlaying: boolean = false;
  private loop: boolean = false;

  constructor(options?: AudioPlayerConfig | undefined) {
    this.player = createAudioPlayer(options);
  }

  /**
   * Load multiple tracks into playlist
   */
  async loadTracks(tracks: string[]): Promise<void> {
    console.log(`📚 Loading ${tracks.length} tracks into playlist...`);

    // Validate all tracks exist and are supported
    for (const track of tracks) {
      const extension = track.split(".").pop()?.toLowerCase();
      if (!extension || !isFormatSupported(extension)) {
        throw new Error(`Unsupported format: ${track}`);
      }

      // Check file exists
      const fs = await import("node:fs");
      if (!fs.existsSync(track)) {
        console.warn(`⚠️  File not found: ${track}`);
      }
    }

    this.tracks = tracks.filter((track) => {
      const fs = require("node:fs");
      return fs.existsSync(track);
    });

    console.log(`✅ Loaded ${this.tracks.length} valid tracks`);
  }

  /**
   * Play current track
   */
  async playCurrentTrack(): Promise<void> {
    if (this.tracks.length === 0) {
      throw new Error("No tracks loaded");
    }

    const currentTrack = this.tracks[this.currentTrackIndex];
    console.log(
      `🎵 Playing track ${this.currentTrackIndex + 1}/${this.tracks.length}: ${currentTrack}`,
    );

    try {
      await this.player.loadFile(currentTrack);
      await this.player.play();
      this.isPlaying = true;

      // Show track metadata
      const metadata = getAudioMetadata(currentTrack);
      console.log("📋 Track info:", metadata);

      // Auto-advance to next track when current one finishes
      this.monitorPlayback();
    } catch (error) {
      console.error("❌ Failed to play track:", error);
      this.isPlaying = false;
    }
  }

  /**
   * Monitor playback and advance to next track
   */
  private monitorPlayback(): void {
    const checkInterval = setInterval(() => {
      if (!this.player.isPlaying() && this.isPlaying) {
        clearInterval(checkInterval);
        this.isPlaying = false;
        this.nextTrack();
      }
    }, 1000);
  }

  /**
   * Play next track in playlist
   */
  async nextTrack(): Promise<void> {
    this.currentTrackIndex++;

    if (this.currentTrackIndex >= this.tracks.length) {
      if (this.loop) {
        this.currentTrackIndex = 0;
        console.log("🔄 Looping playlist");
      } else {
        console.log("✅ End of playlist reached");
        return;
      }
    }

    await this.playCurrentTrack();
  }

  /**
   * Play previous track
   */
  async previousTrack(): Promise<void> {
    this.currentTrackIndex = Math.max(0, this.currentTrackIndex - 1);
    await this.playCurrentTrack();
  }

  /**
   * Pause current playback
   */
  pause(): void {
    this.player.pause();
    this.isPlaying = false;
  }

  /**
   * Resume playback
   */
  async resume(): Promise<void> {
    if (!this.isPlaying && this.tracks.length > 0) {
      await this.player.play();
      this.isPlaying = true;
      this.monitorPlayback();
    }
  }

  /**
   * Stop playback and reset
   */
  async stop(): Promise<void> {
    await this.player.stop();
    this.isPlaying = false;
    this.currentTrackIndex = 0;
  }

  /**
   * Set looping mode
   */
  setLoop(enabled: boolean): void {
    this.loop = enabled;
    console.log(`🔄 Loop mode: ${enabled ? "ON" : "OFF"}`);
  }

  /**
   * Get playlist status
   */
  getStatus() {
    return {
      totalTracks: this.tracks.length,
      currentTrack: this.currentTrackIndex + 1,
      currentTrackPath: this.tracks[this.currentTrackIndex] || null,
      isPlaying: this.isPlaying,
      loop: this.loop,
      volume: this.player.getVolume(),
    };
  }
}

/**
 * Audio visualizer simulation
 */
class AudioVisualizer {
  private bars: number = 20;
  private intervalId: NodeJS.Timeout | null = null;

  start(player: any): void {
    console.log("📊 Starting audio visualizer...");

    this.intervalId = setInterval(() => {
      if (player.isPlaying()) {
        // Simulate audio levels
        const levels = Array.from(
          { length: this.bars },
          () => Math.floor(Math.random() * 20) + 1,
        );

        const visualization = levels
          .map((level) => "█".repeat(level) + "░".repeat(20 - level))
          .join(" ");

        process.stdout.write(`\r🎵 ${visualization}`);
      }
    }, 100);
  }

  stop(): void {
    if (this.intervalId) {
      clearInterval(this.intervalId);
      this.intervalId = null;
      console.log("\n✅ Visualizer stopped");
    }
  }
}

/**
 * Main advanced example runner
 */
async function runAdvancedExample(): Promise<void> {
  console.log("🚀 MiniAudio Node - Advanced TypeScript Examples");
  console.log("=".repeat(60));

  try {
    // Initialize audio system
    console.log("🔧 Initializing audio system...");
    const initResult = initializeAudio();
    console.log("✅", initResult);

    // Example 1: Quick play with presets
    console.log("\n📻 Quick Play Example");
    const quickPlayer = quickPlay(
      process.platform === "win32"
        ? "C:/Windows/Media/tada.wav"
        : process.platform === "darwin"
          ? "/System/Library/Sounds/Glass.aiff"
          : "/usr/share/sounds/alsa/Front_Left.wav",
      { volume: 0.8, autoPlay: true },
    );
    await new Promise((resolve) => setTimeout(resolve, 2000));
    await quickPlayer.stop();

    // Example 2: Playlist management
    console.log("\n🎵 Playlist Management Example");
    const playlist = new PlaylistManager({ volume: 0.7, autoPlay: true });

    // Load system sounds (platform-specific)
    const systemSounds =
      process.platform === "win32"
        ? [
            "C:/Windows/Media/tada.wav",
            "C:/Windows/Media/chimes.wav",
            "C:/Windows/Media/notify.wav",
          ]
        : [];

    const validSounds = systemSounds.filter((sound) => {
      const fs = require("node:fs");
      return fs.existsSync(sound);
    });

    if (validSounds.length > 0) {
      await playlist.loadTracks(validSounds);
      playlist.setLoop(true);
      await playlist.playCurrentTrack();

      // Let it play for a bit
      await new Promise((resolve) => setTimeout(resolve, 5000));
      await playlist.stop();
    } else {
      console.log("⚠️  No system sounds found for this platform");
    }

    // Example 3: Audio effects
    console.log("\n🎭 Audio Effects Example");
    const effectsPlayer = createAudioPlayer({ volume: 0.6 });

    if (validSounds.length > 0) {
      await effectsPlayer.loadFile(validSounds[0]);
    }

    // Example 4: Visualizer
    const visualizer = new AudioVisualizer();
    console.log("\n📊 Audio Visualizer Example");
    const vizPlayer = createAudioPlayer({ volume: 0.5 });

    if (validSounds.length > 0) {
      await vizPlayer.loadFile(validSounds[0]);
      visualizer.start(vizPlayer);
      await vizPlayer.play();

      await new Promise((resolve) => setTimeout(resolve, 5000));
      visualizer.stop();
      await vizPlayer.stop();
    }

    // Example 5: Device enumeration with types
    console.log("\n🔊 Device Enumeration");
    const devicePlayer = createAudioPlayer();
    const devices = devicePlayer.getDevices();

    console.log("Available audio devices:");
    devices.forEach((device: any, index: number) => {
      console.log(
        `  ${index + 1}. ${device.name} (${device.is_default ? "Default" : "Secondary"})`,
      );
    });

    // Example 6: Error handling and validation
    console.log("\n🛡️  Error Handling Example");
    try {
      const errorPlayer = new AudioPlayer();

      // Try to load non-existent file
      await errorPlayer.loadFile("non-existent-file.mp3");
    } catch (error) {
      console.log("✅ Caught expected error:", (error as Error).message);
    }

    try {
      // Try to set invalid volume
      const volumePlayer = new AudioPlayer();
      await volumePlayer.setVolume(2.0); // Invalid: > 1.0
    } catch (error) {
      console.log("✅ Caught volume error:", (error as Error).message);
    }

    console.log("\n🎉 All advanced examples completed!");
    console.log("\n💡 Advanced Features Demonstrated:");
    console.log("  ✓ Type-safe audio operations");
    console.log("  ✓ Playlist management");
    console.log("  ✓ Audio effects (fade, oscillate)");
    console.log("  ✓ Audio visualization simulation");
    console.log("  ✓ Comprehensive error handling");
    console.log("  ✓ Device enumeration with types");
    console.log("  ✓ Async/await patterns");
  } catch (error) {
    console.error("❌ Advanced example failed:", error);
  }
}

// Export for individual testing
export { PlaylistManager, AudioVisualizer, runAdvancedExample };

// Run if this is main module
if (import.meta.main) {
  runAdvancedExample().catch(console.error);
}
