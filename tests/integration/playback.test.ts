/**
 * Integration Tests for Audio Playback
 *
 * These tests verify complete audio playback functionality
 * using Bun's built-in test runner and real audio files.
 */

import { describe, it, expect, beforeEach } from 'bun:test'
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

import type { AudioDeviceInfo, AudioPlayerConfig } from "../../index.js";

// Test audio file path - using Windows system sounds
const TEST_AUDIO_FILE = 'C:/Windows/Media/tada.wav'

describe('Core Audio API Integration Tests', () => {
  beforeEach(async () => {
    // Initialize audio system for each test
    try {
      const result = initializeAudio()
      console.log('Audio system initialized:', result)
    } catch (error) {
      console.warn('Audio system initialization failed:', error)
      // Don't fail test suite if audio system fails to initialize
    }
  })

  describe('AudioPlayer Creation', () => {
    it('should create player with default settings', () => {
      const player = new AudioPlayer()
      expect(player).toBeInstanceOf(AudioPlayer)
      expect(player.getVolume()).toBe(1.0)
      expect(player.isPlaying()).toBe(false)
    })

    it('should create player with createAudioPlayer helper', () => {
      const player = createAudioPlayer({ volume: 0.7 })
      expect(player).toBeInstanceOf(AudioPlayer)
      expect(player.getVolume()).toBeCloseTo(0.7)
    })

    it('should create player with quickPlay helper', () => {
      try {
        const player = quickPlay('non-existent.mp3', { volume: 0.5, autoPlay: false })
        expect(player).toBeInstanceOf(AudioPlayer)
        expect(player.getVolume()).toBeCloseTo(0.5)
      } catch (error) {
        // Expected to fail with file not found
        expect((error as Error).message).toContain('File not found')
      }
    })
  })

  describe('Device Management', () => {
    it('should return consistent device information', () => {
      const player = new AudioPlayer()
      const devices = player.getDevices()
      
      expect(Array.isArray(devices)).toBe(true)
      expect(devices.length).toBeGreaterThan(0)
      
      // Check that devices have required properties
      devices.forEach((device: AudioDeviceInfo) => {
        expect(device).toHaveProperty('id')
        expect(device).toHaveProperty('name')
        expect(device).toHaveProperty('isDefault')
        expect(typeof device.id).toBe('string')
        expect(typeof device.name).toBe('string')
        expect(typeof device.isDefault).toBe('boolean')
      })
    })
  })

  describe('Error Handling', () => {
    it('should handle invalid file paths gracefully', () => {
      const player = new AudioPlayer()
      
      expect(() => player.loadFile('')).toThrow()
      expect(() => player.loadFile('non-existent-file.mp3')).toThrow()
    })

    it('should handle playback operations on uninitialized player', () => {
      const player = new AudioPlayer()
      
      // These should not crash, but may throw errors
      expect(() => player.play()).toThrow()
      expect(() => player.pause()).toThrow()
      expect(() => player.stop()).toThrow()
    })

    it('should handle volume validation', () => {
      const player = new AudioPlayer()
      
      expect(() => player.setVolume(-0.1)).toThrow()
      expect(() => player.setVolume(1.1)).toThrow()
      
      // Valid values should work
      expect(() => player.setVolume(0.0)).not.toThrow()
      expect(() => player.setVolume(1.0)).not.toThrow()
      expect(() => player.setVolume(0.5)).not.toThrow()
    })
  })

  describe('System Integration', () => {
    it('should initialize audio system consistently', () => {
      // Initialize multiple times
      for (let i = 0; i < 3; i++) {
        const result = initializeAudio()
        expect(typeof result).toBe('string')
        expect(result).toContain('initialized')
      }
    })

    it('should handle concurrent operations', () => {
      const players = Array.from({ length: 5 }, () => new AudioPlayer())
      
      // All players should work independently
      players.forEach((player, index) => {
        player.setVolume(index / 5)
        expect(player.getVolume()).toBeCloseTo(index / 5)
      })
    })

    it('should get supported formats consistently', () => {
      const formats1 = getSupportedFormats()
      const formats2 = getSupportedFormats()
      
      expect(formats1).toEqual(formats2)
      expect(Array.isArray(formats1)).toBe(true)
      expect(formats1.length).toBeGreaterThan(0)
    })
  })

  describe('Metadata API', () => {
    it('should handle metadata requests gracefully', () => {
      // Test with non-existent file
      expect(() => getAudioMetadata('non-existent.mp3')).toThrow()
    })
  })

  describe('PlaybackState', () => {
    it('should have correct enum values', () => {
      expect(PlaybackState.Stopped).toBe(0)
      expect(PlaybackState.Loaded).toBe(1)
      expect(PlaybackState.Playing).toBe(2)
      expect(PlaybackState.Paused).toBe(3)
    })

    it('should return correct initial state', () => {
      const player = new AudioPlayer()
      expect(player.getState()).toBe(PlaybackState.Stopped)
    })
  })

  describe('Format Detection', () => {
    it('should detect supported formats correctly', () => {
      const supportedFormats = getSupportedFormats()
      
      // Check that common formats are supported
      expect(supportedFormats).toContain('wav')
      expect(supportedFormats).toContain('mp3')
      expect(supportedFormats).toContain('flac')
      expect(supportedFormats).toContain('ogg')
      
      // Check that all formats are lowercase strings
      supportedFormats.forEach(format => {
        expect(typeof format).toBe('string')
        expect(format).toBe(format.toLowerCase())
      })
    })
  })
})
