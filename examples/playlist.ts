/**
 * Playlist Example with TypeScript and Bun
 *
 * This example demonstrates how to:
 * 1. Create and manage playlists
 * 2. Load multiple audio files
 * 3. Control playback (play, pause, stop, next, previous)
 * 4. Set volume and other properties
 * 5. Handle playlist events
 */

import { AudioPlayer, AudioPlayerConfig, getSupportedFormats } from "../../index.js";
import { existsSync, statSync } from "node:fs";

interface PlaylistTrack {
  path: string;
  name: string;
  duration: number;
  index: number;
}

class Playlist {
  private tracks: PlaylistTrack[] = [];
  private currentIndex: number = 0;
  private player: AudioPlayer;
  private isPlaying: boolean = false;
  private isPaused: boolean = false;
  private volume: number = 1.0;
  private loop: 'none' | 'all' | 'one' = 'none';
  private shuffle: boolean = false;

  constructor(config?: AudioPlayerConfig) {
    this.player = new AudioPlayer();
    if (config?.volume !== undefined) {
      this.volume = config.volume;
      this.player.setVolume(config.volume);
    }
  }

  /**
   * Add a single track to the playlist
   */
  async addTrack(filePath: string): Promise<boolean> {
    try {
      // Validate file
      if (!existsSync(filePath)) {
        console.warn(`⚠️  File not found: ${filePath}`);
        return false;
      }

      const stats = statSync(filePath);
      if (stats.isDirectory()) {
        console.warn(`⚠️  Is directory: ${filePath}`);
        return false;
      }

      // Get file name
      const path = require("node:path");
      const name = path.basename(filePath);

      // Try to load the file to get duration
      let duration = 0;
      try {
        this.player.loadFile(filePath);
        duration = this.player.getDuration();
        this.player.stop(); // Stop after checking
      } catch (error) {
        console.warn(`⚠️  Could not load file: ${name}`);
        return false;
      }

      // Add to playlist
      const track: PlaylistTrack = {
        path: filePath,
        name: name,
        duration: duration,
        index: this.tracks.length,
      };

      this.tracks.push(track);
      console.log(`✅ Added: ${name} (${duration.toFixed(2)}s)`);
      return true;

    } catch (error) {
      console.error(`❌ Failed to add track:`, (error as Error).message);
      return false;
    }
  }

  /**
   * Add multiple tracks to the playlist
   */
  async addTracks(filePaths: string[]): Promise<number> {
    let added = 0;
    console.log(`📝 Loading ${filePaths.length} tracks...`);

    for (const path of filePaths) {
      if (await this.addTrack(path)) {
        added++;
      }
    }

    console.log(`✅ Added ${added}/${filePaths.length} tracks to playlist`);
    return added;
  }

  /**
   * Remove track at index
   */
  removeTrack(index: number): void {
    if (index < 0 || index >= this.tracks.length) {
      throw new Error(`Invalid track index: ${index}`);
    }

    const removed = this.tracks.splice(index, 1);
    console.log(`🗑️  Removed: ${removed[0].name}`);

    // Adjust current index if needed
    if (this.currentIndex >= index && this.currentIndex > 0) {
      this.currentIndex--;
    }
  }

  /**
   * Clear the playlist
   */
  clear(): void {
    const count = this.tracks.length;
    this.tracks = [];
    this.currentIndex = 0;
    this.isPlaying = false;
    this.isPaused = false;
    console.log(`🧹 Cleared ${count} tracks from playlist`);
  }

  /**
   * Get track by index
   */
  getTrack(index: number): PlaylistTrack | null {
    return this.tracks[index] || null;
  }

  /**
   * Get current track
   */
  getCurrentTrack(): PlaylistTrack | null {
    if (this.tracks.length === 0) return null;
    return this.tracks[this.currentIndex];
  }

  /**
   * Get all tracks
   */
  getTracks(): PlaylistTrack[] {
    return [...this.tracks];
  }

  /**
   * Get total duration of playlist
   */
  getTotalDuration(): number {
    return this.tracks.reduce((sum, track) => sum + track.duration, 0);
  }

  /**
   * Play current track
   */
  async play(index?: number): Promise<void> {
    if (this.tracks.length === 0) {
      throw new Error("Playlist is empty");
    }

    if (index !== undefined) {
      if (index < 0 || index >= this.tracks.length) {
        throw new Error(`Invalid track index: ${index}`);
      }
      this.currentIndex = index;
      this.stop();
    }

    const track = this.getCurrentTrack();
    if (!track) {
      throw new Error("No current track");
    }

    try {
      console.log(`🎵 Playing: ${track.name} (${track.duration.toFixed(2)}s)`);
      this.player.loadFile(track.path);
      this.player.setVolume(this.volume);
      this.player.play();
      this.isPlaying = true;
      this.isPaused = false;

      // Set up auto-advance if needed
      if (this.loop === 'one') {
        this.setupAutoAdvanceOne();
      } else if (this.loop === 'all' || this.shuffle) {
        this.setupAutoAdvance();
      }

    } catch (error) {
      this.isPlaying = false;
      throw error;
    }
  }

  /**
   * Pause playback
   */
  pause(): void {
    if (this.isPlaying && !this.isPaused) {
      this.player.pause();
      this.isPaused = true;
      console.log("⏸️  Paused");
    }
  }

  /**
   * Resume playback
   */
  resume(): void {
    if (this.isPaused) {
      this.player.play();
      this.isPaused = false;
      console.log("▶️  Resumed");
    }
  }

  /**
   * Stop playback
   */
  stop(): void {
    if (this.isPlaying) {
      this.player.stop();
      this.isPlaying = false;
      this.isPaused = false;
      console.log("⏹️  Stopped");
    }
  }

  /**
   * Play next track
   */
  async next(): Promise<void> {
    if (this.tracks.length === 0) return;

    let nextIndex: number;

    if (this.shuffle) {
      // Random index
      nextIndex = Math.floor(Math.random() * this.tracks.length);
    } else {
      nextIndex = this.currentIndex + 1;
      if (nextIndex >= this.tracks.length) {
        if (this.loop === 'all') {
          nextIndex = 0;
        } else {
          console.log("🏁 End of playlist");
          return;
        }
      }
    }

    await this.play(nextIndex);
  }

  /**
   * Play previous track
   */
  async previous(): Promise<void> {
    if (this.tracks.length === 0) return;

    let prevIndex = this.currentIndex - 1;
    if (prevIndex < 0) {
      if (this.loop === 'all') {
        prevIndex = this.tracks.length - 1;
      } else {
        console.log("⏮️  Already at start");
        return;
      }
    }

    await this.play(prevIndex);
  }

  /**
   * Set loop mode
   */
  setLoop(mode: 'none' | 'all' | 'one'): void {
    this.loop = mode;
    console.log(`🔁 Loop mode set to: ${mode}`);
  }

  /**
   * Toggle shuffle
   */
  setShuffle(enabled: boolean): void {
    this.shuffle = enabled;
    console.log(`🔀 Shuffle ${enabled ? 'ON' : 'OFF'}`);
  }

  /**
   * Set volume (0.0 to 1.0)
   */
  setVolume(volume: number): void {
    if (volume < 0 || volume > 1) {
      throw new Error("Volume must be between 0.0 and 1.0");
    }
    this.volume = volume;
    this.player.setVolume(volume);
    console.log(`🔊 Volume: ${(volume * 100).toFixed(0)}%`);
  }

  /**
   * Get volume
   */
  getVolume(): number {
    return this.volume;
  }

  /**
   * Get current track position
   */
  getPlaybackTime(): { current: number; total: number; percent: number } | null {
    const track = this.getCurrentTrack();
    if (!track) return null;

    const current = this.player.getCurrentTime();
    const total = track.duration;
    const percent = total > 0 ? (current / total) * 100 : 0;

    return { current, total, percent };
  }

  /**
   * Check if playing
   */
  isCurrentlyPlaying(): boolean {
    return this.isPlaying && !this.isPaused;
  }

  /**
   * Check if paused
   */
  isCurrentlyPaused(): boolean {
    return this.isPaused;
  }

  /**
   * Get playlist status
   */
  getStatus() {
    const current = this.getCurrentTrack();
    return {
      tracks: this.tracks.length,
      currentIndex: this.currentIndex,
      currentTrack: current?.name || null,
      currentPath: current?.path || null,
      isPlaying: this.isPlaying,
      isPaused: this.isPaused,
      volume: this.volume,
      loop: this.loop,
      shuffle: this.shuffle,
      totalDuration: this.getTotalDuration(),
    };
  }

  /**
   * Print playlist info
   */
  printPlaylist(): void {
    console.log("\n📋 Playlist:");
    console.log("=".repeat(60));

    if (this.tracks.length === 0) {
      console.log("   (empty)");
      return;
    }

    this.tracks.forEach((track, index) => {
      const prefix = index === this.currentIndex ? "▶️  " : "    ";
      const playerStatus = index === this.currentIndex && this.isPlaying ? "[Playing] " : "";
      const duration = track.duration.toFixed(2);
      console.log(`${prefix}${index + 1}. ${playerStatus}${track.name} (${duration}s)`);
    });

    console.log("=".repeat(60));
    console.log(`Total: ${this.tracks.length} tracks, ${this.getTotalDuration().toFixed(2)}s`);
  }

  /**
   * Monitor playback progress
   */
  private setupAutoAdvance(): void {
    // For single track playback with auto-next
    // This is a simplified version
  }

  private setupAutoAdvanceOne(): void {
    // For looping single track
    // This is a simplified version
  }

  /**
   * Clean up resources
   */
  dispose(): void {
    this.stop();
    this.player = null as any;
  }
}

/**
 * Helper function to get audio files from a directory
 */
function getAudioFiles(directory: string): string[] {
  const fs = require("node:fs");
  const path = require("node:fs").path;

  const supportedFormats = new Set(getSupportedFormats().map(f => f.toLowerCase()));
  const audioFiles: string[] = [];

  try {
    const items = fs.readdirSync(directory);
    for (const item of items) {
      const fullPath = path.join(directory, item);
      const stat = fs.statSync(fullPath);

      if (stat.isFile()) {
        const ext = path.extname(item).toLowerCase().slice(1);
        if (supportedFormats.has(ext)) {
          audioFiles.push(fullPath);
        }
      }
    }
  } catch (error) {
    console.error(`Failed to scan directory: ${directory}`);
  }

  return audioFiles;
}

/**
 * Demonstrate playlist functionality
 */
async function demonstratePlaylist(): Promise<void> {
  console.log("🎵 Playlist Example with TypeScript and Bun");
  console.log("=".repeat(60));

  // Create playlist with configuration
  console.log("\n🎧 Creating playlist...");
  const playlist = new Playlist({ volume: 0.8 });

  // Get platform-specific audio files
  const audioFiles = getPlatformAudioFiles();

  if (audioFiles.length === 0) {
    console.log("\n⚠️  No audio files found for this platform.");
    console.log("   You can test by providing a directory path as argument:");
    console.log("   bun ./examples/typescript/playlist.ts /path/to/audio/files");

    // Check if directory provided as argument
    const directory = process.argv[2];
    if (directory) {
      console.log(`\n🔍 Scanning directory: ${directory}`);
      const files = getAudioFiles(directory);

      if (files.length > 0) {
        await playlist.addTracks(files);
        playlist.printPlaylist();
        await runPlaylistDemo(playlist);
      } else {
        console.log("❌ No audio files found in directory");
      }
    }
    return;
  }

  // Add files to playlist
  await playlist.addTracks(audioFiles);
  playlist.printPlaylist();

  // Run demo playback
  await runPlaylistDemo(playlist);

  // Clean up
  playlist.dispose();
}

/**
 * Run playlist demo with controls
 */
async function runPlaylistDemo(playlist: Playlist): Promise<void> {
  if (playlist.getTracks().length === 0) {
    return;
  }

  console.log("\n🎯 Starting playlist demo...");
  console.log("=".repeat(60));

  try {
    // Play first track
    await playlist.play(0);
    console.log("   Playing first track...");

    // Wait a bit, then show controls
    await new Promise(resolve => setTimeout(resolve, 2000));

    // Control 1: Adjust volume
    console.log("\n🔉 Adjusting volume...");
    playlist.setVolume(0.5);
    await new Promise(resolve => setTimeout(resolve, 1500));

    // Control 2: Pause
    console.log("\n⏸️  Pausing playback...");
    playlist.pause();
    await new Promise(resolve => setTimeout(resolve, 1000));

    // Control 3: Resume
    console.log("\n▶️  Resuming playback...");
    playlist.resume();
    await new Promise(resolve => setTimeout(resolve, 2000));

    // Control 4: Volume up
    console.log("\n🔊 Increasing volume...");
    playlist.setVolume(1.0);
    await new Promise(resolve => setTimeout(resolve, 1500));

    // Control 5: Next track (if available)
    if (playlist.getTracks().length > 1) {
      console.log("\n⏭️  Moving to next track...");
      await playlist.next();
      await new Promise(resolve => setTimeout(resolve, 2000));
    }

    // Control 6: Previous track
    if (playlist.getTracks().length > 1) {
      console.log("\n⏮️  Moving to previous track...");
      await playlist.previous();
      await new Promise(resolve => setTimeout(resolve, 1500));
    }

    // Control 7: Set shuffle
    console.log("\n🔀 Enabling shuffle...");
    playlist.setShuffle(true);
    await new Promise(resolve => setTimeout(resolve, 500));

    // Control 8: Set loop mode
    console.log("\n🔁 Setting loop to 'all'...");
    playlist.setLoop('all');
    await new Promise(resolve => setTimeout(resolve, 500));

    // Control 9: Show status
    console.log("\n📊 Playlist Status:");
    console.log(playlist.getStatus());
    playlist.printPlaylist();

    // Control 10: Stop
    console.log("\n⏹️  Stopping playback...");
    playlist.stop();

    console.log("\n✅ Playlist demo completed!");
    console.log("\n💡 Commands demonstrated:");
    console.log("  ✓ Play specific track");
    console.log("  ✓ Adjust volume");
    console.log("  ✓ Pause and resume");
    console.log("  ✓ Next and previous tracks");
    console.log("  ✓ Shuffle mode");
    console.log("  ✓ Loop modes");
    console.log("  ✓ Status and playlist display");

  } catch (error) {
    console.error("❌ Playlist demo error:", (error as Error).message);
  }
}

/**
 * Get platform-specific audio files
 */
function getPlatformAudioFiles(): string[] {
  const fs = require("node:fs");
  const platform = process.platform;
  const files: string[] = [];

  if (platform === "win32") {
    const sounds = [
      "C:/Windows/Media/tada.wav",
      "C:/Windows/Media/chimes.wav",
      "C:/Windows/Media/notify.wav"
    ];
    for (const sound of sounds) {
      if (fs.existsSync(sound)) files.push(sound);
    }
  } else if (platform === "darwin") {
    const sounds = [
      "/System/Library/Sounds/Glass.aiff",
      "/System/Library/Sounds/Guir.aiff",
      "/System/Library/Sounds/Sosumi.aiff"
    ];
    for (const sound of sounds) {
      if (fs.existsSync(sound)) files.push(sound);
    }
  } else if (platform === "linux") {
    const soundDirs = [
      "/usr/share/sounds/alsa",
      "/usr/share/sounds/gnome/default",
      "/usr/share/sounds/ubuntu"
    ];
    for (const dir of soundDirs) {
      if (fs.existsSync(dir)) {
        try {
          const items = fs.readdirSync(dir);
          items.forEach((item: string) => {
            const fullPath = `${dir}/${item}`;
            if (fs.statSync(fullPath).isFile()) {
              files.push(fullPath);
            }
          });
        } catch (e) {}
        if (files.length > 0) break;
      }
    }
  }

  return files.slice(0, 5); // Limit to 5 files for demo
}

/**
 * Main function
 */
async function runPlaylistExample(): Promise<void> {
  try {
    // Note: initializeAudio() is not needed for playlist since AudioPlayer handles it internally
    await demonstratePlaylist();
  } catch (error) {
    console.error("\n❌ Fatal error:", (error as Error).message);
    console.error((error as Error).stack);
    process.exit(1);
  }
}

// Run if this is the main module
if (import.meta.main) {
  runPlaylistExample();
}

export { Playlist, runPlaylistExample };
