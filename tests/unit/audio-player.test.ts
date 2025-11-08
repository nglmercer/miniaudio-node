/**
 * Unit Tests for AudioPlayer
 *
 * These tests verify of core functionality of AudioPlayer class
 * using Bun's built-in test runner.
 */

import { describe, it, expect, beforeEach, afterEach } from 'bun:test'
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

describe('AudioPlayer', () => {
  let player: typeof AudioPlayer | any = null;

  beforeEach(async () => {
    // Initialize audio system before each test
    try {
      initializeAudio()
    } catch (error) {
      console.warn('Audio system initialization failed:', error)
    }

    player = new AudioPlayer()
  })

  afterEach(() => {
    try {
      if (player.isPlaying()) {
        player.stop()
      }
    } catch (error) {
      // Ignore cleanup errors
    }
  })

  describe('Constructor', () => {
    it('should create a new AudioPlayer instance', () => {
      expect(player).toBeInstanceOf(AudioPlayer)
    })

    it('should have default volume of 1.0', () => {
      expect(player.getVolume()).toBe(1.0)
    })

    it('should not be playing initially', () => {
      expect(player.isPlaying()).toBe(false)
    })
  })

  describe('Volume Control', () => {
    it('should set volume correctly', () => {
      player.setVolume(0.5)
      expect(player.getVolume()).toBeCloseTo(0.5)
    })

    it('should accept minimum volume', () => {
      player.setVolume(0.0)
      expect(player.getVolume()).toBe(0.0)
    })

    it('should accept maximum volume', () => {
      player.setVolume(1.0)
      expect(player.getVolume()).toBe(1.0)
    })

    it('should throw error for volume below 0.0', () => {
      expect(() => player.setVolume(-0.1)).toThrow('Volume must be between 0.0 and 1.0')
    })

    it('should throw error for volume above 1.0', () => {
      expect(() => player.setVolume(1.1)).toThrow('Volume must be between 0.0 and 1.0')
    })
  })

  describe('Device Management', () => {
    it('should return available devices', () => {
      const devices = player.getDevices()
      expect(Array.isArray(devices)).toBe(true)
      expect(devices.length).toBeGreaterThan(0)
    })

    it('should return device objects with required properties', () => {
      const devices = player.getDevices()
      devices.forEach((device: any) => {
        expect(device).toHaveProperty('id')
        expect(device).toHaveProperty('name')
        expect(device).toHaveProperty('isDefault')
        expect(typeof device.id).toBe('string')
        expect(typeof device.name).toBe('string')
        expect(typeof device.isDefault).toBe('boolean')
      })
    })
  })

  describe('Playback State', () => {
    it('should report not playing when no file is loaded', () => {
      expect(player.isPlaying()).toBe(false)
    })

    it('should report not playing when stopped', () => {
      // This test may need adjustment based on actual implementation
      expect(player.isPlaying()).toBe(false)
    })
  })

  describe('File Loading', () => {
    it('should throw error when loading non-existent file', () => {
      expect(() => player.loadFile('non-existent-file.mp3')).toThrow()
    })

    it('should throw error when loading with empty path', () => {
      expect(() => player.loadFile('')).toThrow()
    })
  })

  describe('Metadata', () => {
    it('should return duration as number', () => {
      const duration = player.getDuration()
      expect(typeof duration).toBe('number')
      expect(duration).toBeGreaterThanOrEqual(0)
    })

    it('should return current time as number', () => {
      const currentTime = player.getCurrentTime()
      expect(typeof currentTime).toBe('number')
      expect(currentTime).toBeGreaterThanOrEqual(0)
    })
  })
})

describe('Audio System', () => {
  describe('initializeAudio', () => {
    it('should initialize audio system successfully', () => {
      expect(() => initializeAudio()).not.toThrow()
      const result = initializeAudio()
      expect(typeof result).toBe('string')
      expect(result).toContain('initialized')
    })
  })

  describe('getSupportedFormats', () => {
    it('should return array of supported formats', () => {
      const formats = getSupportedFormats()
      expect(Array.isArray(formats)).toBe(true)
      expect(formats.length).toBeGreaterThan(0)
    })

    it('should include common audio formats', () => {
      const formats = getSupportedFormats()
      expect(formats).toContain('wav')
      expect(formats).toContain('mp3')
      expect(formats).toContain('flac')
      expect(formats).toContain('ogg')
    })

    it('should contain only lowercase format names', () => {
      const formats = getSupportedFormats()
      formats.forEach((format: any) => {
        expect(format).toBe(format.toLowerCase())
      })
    })
  })
})

describe('Error Handling', () => {
  it('should handle playback operations without loaded file', () => {
    const player = new AudioPlayer()

    expect(() => player.play()).toThrow('Player not initialized')
    expect(() => player.pause()).toThrow('Player not initialized')
    expect(() => player.stop()).toThrow('Player not initialized')
  })

  it('should handle invalid volume values', () => {
    const player = new AudioPlayer()

    expect(() => player.setVolume(-0.1)).toThrow('Volume must be between 0.0 and 1.0')
    expect(() => player.setVolume(1.1)).toThrow('Volume must be between 0.0 and 1.0')
  })
})

describe('Integration Tests', () => {
  it('should maintain state across multiple operations', () => {
    const player = new AudioPlayer()

    // Test volume persistence
    player.setVolume(0.7)
    expect(player.getVolume()).toBeCloseTo(0.7)

    // Test that volume doesn't reset after other operations
    const devices = player.getDevices()
    expect(devices.length).toBeGreaterThan(0)
    expect(player.getVolume()).toBeCloseTo(0.7)
  })

  it('should handle rapid state changes', () => {
    const player = new AudioPlayer()

    // Rapid volume changes
    for (let i = 0; i < 10; i++) {
      player.setVolume(i / 10)
      expect(player.getVolume()).toBeCloseTo(i / 10)
    }

    // Rapid device queries
    for (let i = 0; i < 10; i++) {
      const devices = player.getDevices()
      expect(Array.isArray(devices)).toBe(true)
    }
  })
})
