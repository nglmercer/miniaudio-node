/**
 * MiniAudio Node - High-performance native audio playback for Bun/Node.js
 *
 * Built with Rust and NAPI-RS, providing cross-platform audio playback
 * with support for multiple formats including WAV, MP3, FLAC, OGG, M4A, and AAC.
 *
 * @author Audio Development Team
 * @version 1.0.0
 * @license MIT
 */

// Re-export all types and functionality from the native module
export * from '../dist/index.js';

// Additional TypeScript utilities and helpers
export interface AudioPlayerOptions {
  volume?: number;
  loop?: boolean;
  autoPlay?: boolean;
}

export interface PlaylistOptions extends AudioPlayerOptions {
  shuffle?: boolean;
}

export interface AudioMetadata {
  filePath: string;
  fileSize?: string;
  duration?: string;
  sampleRate?: string;
  channels?: string;
  bitrate?: string;
  format?: string;
}

/**
 * Enhanced AudioPlayer wrapper with additional utilities
 */
export class EnhancedAudioPlayer {
  private player: any;
  private options: AudioPlayerOptions;

  constructor(options: AudioPlayerOptions = {}) {
    // Import the native AudioPlayer dynamically
    const native = require('../dist/index.js');
    this.options = { ...options };
    this.player = new native.AudioPlayer();

    // Apply initial configuration
    if (options.volume !== undefined) {
      this.setVolume(options.volume);
    }
  }

  /**
   * Load an audio file
   */
  async loadFile(filePath: string): Promise<void> {
    return this.player.loadFile(filePath);
  }

  /**
   * Play the loaded audio
   */
  async play(): Promise<void> {
    return this.player.play();
  }

  /**
   * Pause the current playback
   */
  async pause(): Promise<void> {
    return this.player.pause();
  }

  /**
   * Stop playback
   */
  async stop(): Promise<void> {
    return this.player.stop();
  }

  /**
   * Set volume (0.0 to 1.0)
   */
  async setVolume(volume: number): Promise<void> {
    if (volume < 0 || volume > 1) {
      throw new Error('Volume must be between 0.0 and 1.0');
    }
    return this.player.setVolume(volume);
  }

  /**
   * Get current volume
   */
  getVolume(): number {
    return this.player.getVolume();
  }

  /**
   * Check if currently playing
   */
  isPlaying(): boolean {
    return this.player.isPlaying();
  }

  /**
   * Get playback state
   */
  getState(): any {
    return this.player.getState();
  }

  /**
   * Get current file path
   */
  getCurrentFile(): string | null {
    return this.player.getCurrentFile();
  }

  /**
   * Get available devices
   */
  getDevices(): any[] {
    return this.player.getDevices();
  }

  /**
   * Fade in effect
   */
  async fadeIn(duration: number = 2000): Promise<void> {
    const steps = 20;
    const stepDuration = duration / steps;
    const targetVolume = this.options.volume || 1.0;
    const volumeIncrement = targetVolume / steps;

    this.player.setVolume(0);
    this.player.play();

    for (let i = 1; i <= steps; i++) {
      await new Promise(resolve => setTimeout(resolve, stepDuration));
      this.player.setVolume(i * volumeIncrement);
    }
  }

  /**
   * Fade out effect
   */
  async fadeOut(duration: number = 2000): Promise<void> {
    const steps = 20;
    const stepDuration = duration / steps;
    const currentVolume = this.getVolume();
    const volumeDecrement = currentVolume / steps;

    for (let i = 1; i <= steps; i++) {
      await new Promise(resolve => setTimeout(resolve, stepDuration));
      this.player.setVolume(currentVolume - i * volumeDecrement);
    }

    this.player.pause();
    this.player.setVolume(currentVolume);
  }
}

/**
 * Utility function to create an enhanced audio player
 */
export function createEnhancedPlayer(options?: AudioPlayerOptions): EnhancedAudioPlayer {
  return new EnhancedAudioPlayer(options);
}

/**
 * Quick play with enhanced features
 */
export async function enhancedQuickPlay(
  filePath: string,
  options: AudioPlayerOptions = {}
): Promise<EnhancedAudioPlayer> {
  const player = new EnhancedAudioPlayer(options);
  await player.loadFile(filePath);

  if (options.autoPlay) {
    await player.play();
  }

  return player;
}

/**
 * Parse audio metadata from JSON string
 */
export function parseAudioMetadata(metadataJson: string): AudioMetadata {
  try {
    return JSON.parse(metadataJson);
  } catch (error) {
    throw new Error(`Failed to parse audio metadata: ${error}`);
  }
}

/**
 * Check if audio file format is supported
 */
export function isAudioFormatSupported(filePath: string): boolean {
  const extension = filePath.split('.').pop()?.toLowerCase();
  if (!extension) return false;

  const native = require('../dist/index.js');
  return native.isFormatSupported(extension);
}

/**
 * Get supported audio formats
 */
export function getSupportedAudioFormats(): string[] {
  const native = require('../dist/index.js');
  return native.getSupportedFormats();
}

/**
 * Initialize audio system with enhanced error handling
 */
export async function initializeAudioSystem(): Promise<string> {
  const native = require('../dist/index.js');
  try {
    return await native.initializeAudio();
  } catch (error) {
    throw new Error(`Failed to initialize audio system: ${error}`);
  }
}

// Export default for convenience
export default {
  // Native exports
  AudioPlayer: require('../dist/index.js').AudioPlayer,
  PlaybackState: require('../dist/index.js').PlaybackState,
  initializeAudio: require('../dist/index.js').initializeAudio,
  getSupportedFormats: require('../dist/index.js').getSupportedFormats,
  isFormatSupported: require('../dist/index.js').isFormatSupported,
  getAudioInfo: require('../dist/index.js').getAudioInfo,
  testTone: require('../dist/index.js').testTone,
  createAudioPlayer: require('../dist/index.js').createAudioPlayer,
  quickPlay: require('../dist/index.js').quickPlay,
  getAudioMetadata: require('../dist/index.js').getAudioMetadata,

  // Enhanced exports
  EnhancedAudioPlayer,
  createEnhancedPlayer,
  enhancedQuickPlay,
  parseAudioMetadata,
  isAudioFormatSupported,
  getSupportedAudioFormats,
  initializeAudioSystem,
};
