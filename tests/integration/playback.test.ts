/**
 * Integration Tests for Core Audio API
 *
 * These tests verify the core functionality without requiring actual audio playback.
 * Tests focus on API validation and basic functionality.
 */

import { describe, it, expect, beforeEach } from 'bun:test'
import {
  AudioPlayer,
  initializeAudio,
  getSupportedFormats,
  createAudioPlayer,
  quickPlay,
  isFormatSupported,
  getAudioMetadata,
  PlaybackState
} from '../../dist/index.js'

describe('Core Audio API Integration Tests', () => {
  beforeEach(() => {
    // Initialize audio system before each test
    try {
      initializeAudio()
    } catch (error) {
      console.warn('Audio system initialization failed:', error)
    }
  })

  describe('Format Detection', () => {
    it('should detect supported formats correctly', () => {
      const supportedFormats = getSupportedFormats()

      expect(isFormatSupported('wav')).toBe(true)
      expect(isFormatSupported('mp3')).toBe(true)
      expect(isFormatSupported('flac')).toBe(true)
      expect(isFormatSupported('ogg')).toBe(true)
      expect(isFormatSupported('m4a')).toBe(true)
      expect(isFormatSupported('aac')).toBe(true)
      expect(isFormatSupported('unknown')).toBe(false)
      expect(isFormatSupported('')).toBe(false)
    })
  })

  describe('AudioPlayer Creation', () => {
    it('should create player with default settings', () => {
      const player = new AudioPlayer()
      expect(player.getVolume()).toBe(1.0)
      expect(player.isPlaying()).toBe(false)
    })

    it('should create player with createAudioPlayer helper', () => {
      const player = createAudioPlayer({ volume: 0.7 })
      expect(player.getVolume()).toBeCloseTo(0.7)
    })

    it('should create player with quickPlay helper', () => {
      // This will fail with file not found, but should create the player
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

      // Check device structure
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

  describe('Error Handling', () => {
    it('should handle invalid file paths gracefully', () => {
      const player = new AudioPlayer()

      expect(() => player.loadFile('')).toThrow()
      expect(() => player.loadFile('non-existent-file.mp3')).toThrow()
      expect(() => player.loadFile('path/to/invalid.mp3')).toThrow()
    })

    it('should handle playback operations on uninitialized player', () => {
      const player = new AudioPlayer()

      expect(() => player.play()).toThrow('Player not initialized')
      expect(() => player.pause()).toThrow('Player not initialized')
      expect(() => player.stop()).toThrow('Player not initialized')
    })

    it('should handle volume validation', () => {
      const player = new AudioPlayer()

      expect(() => player.setVolume(-0.1)).toThrow('Volume must be between 0.0 and 1.0')
      expect(() => player.setVolume(1.1)).toThrow('Volume must be between 0.0 and 1.0')
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

    it('should handle concurrent operations', async () => {
      const players = Array.from({ length: 3 }, () => new AudioPlayer())

      // Set different volumes for each player
      players.forEach((player, index) => {
        player.setVolume(0.1 + (index * 0.3))
      })

      // Verify each player has different volume
      const volumes = players.map(player => player.getVolume())
      const uniqueVolumes = new Set(volumes)
      expect(uniqueVolumes.size).toBe(players.length)

      // All players should be able to query devices
      players.forEach(player => {
        const devices = player.getDevices()
        expect(Array.isArray(devices)).toBe(true)
        expect(devices.length).toBeGreaterThan(0)
      })
    })

    it('should get supported formats consistently', () => {
      const formats1 = getSupportedFormats()
      const formats2 = getSupportedFormats()

      expect(formats1).toEqual(formats2)
      expect(formats1.length).toBeGreaterThan(0)
      expect(formats1).toContain('wav')
      expect(formats1).toContain('mp3')
      expect(formats1).toContain('flac')
      expect(formats1).toContain('ogg')
    })
  })

  describe('Metadata API', () => {
    it('should handle metadata requests gracefully', () => {
      // Test with non-existent file
      expect(() => getAudioMetadata('non-existent.mp3')).toThrow()

      // Test with empty path
      expect(() => getAudioMetadata('')).toThrow()
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
      expect(player.isPlaying()).toBe(false)
    })
  })
})
