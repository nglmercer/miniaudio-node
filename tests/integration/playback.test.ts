/**
 * Integration Tests for Audio Playback
 *
 * These tests verify the actual audio playback functionality
 * using real audio files and system integration.
 */

import { describe, it, expect, beforeEach, afterEach, test } from 'bun:test'
import {
  AudioPlayer,
  initializeAudio,
  getSupportedFormats,
  createAudioPlayer,
  quickPlay,
  isFormatSupported,
  getAudioMetadata,
  type PlaybackOptions
} from '../../dist/index.js'
import { existsSync } from 'node:fs'
import { join } from 'node:path'
import { tmpdir } from 'node:os'

describe('Audio Playback Integration Tests', () => {
  let player: AudioPlayer
  let testAudioFile: string

  beforeEach(async () => {
    // Initialize audio system
    try {
      const result = initializeAudio()
      expect(result).toContain('initialized')
    } catch (error) {
      console.warn('Audio system initialization failed:', error)
      throw new Error('Audio system must be initialized for integration tests')
    }

    player = new AudioPlayer()

    // Find a test audio file
    testAudioFile = getTestAudioFile()
  })

  afterEach(() => {
    try {
      if (player?.isPlaying()) {
        player.stop()
      }
    } catch (error) {
      // Ignore cleanup errors
    }
  })

  function getTestAudioFile(): string {
    // Try to find system audio files
    const systemFiles = [
      // Windows
      'C:/Windows/Media/tada.wav',
      'C:/Windows/Media/chimes.wav',
      'C:/Windows/Media/notify.wav',
      // macOS
      '/System/Library/Sounds/Glass.aiff',
      '/System/Library/Sounds/Ping.aiff',
      '/System/Library/Sounds/Pop.aiff',
      // Linux
      '/usr/share/sounds/alsa/Front_Left.wav',
      '/usr/share/sounds/alsa/Front_Right.wav',
    ]

    for (const file of systemFiles) {
      if (existsSync(file)) {
        console.log(`Using test audio file: ${file}`)
        return file
      }
    }

    // Create a simple test file if none exist
    console.warn('No system audio files found, integration tests may be limited')
    return ''
  }

  describe('Real Audio File Playback', () => {
    test.skipIf(!testAudioFile)('should load and play real audio file', async () => {
      expect(testAudioFile).toBeTruthy()

      // Load the file
      expect(() => player.loadFile(testAudioFile)).not.toThrow()

      // Check initial state
      expect(player.isPlaying()).toBe(false)

      // Start playback
      player.play()
      expect(player.isPlaying()).toBe(true)

      // Let it play for a moment
      await new Promise(resolve => setTimeout(resolve, 1000))

      // Pause playback
      player.pause()
      expect(player.isPlaying()).toBe(false)

      // Resume playback
      player.play()
      expect(player.isPlaying()).toBe(true)

      // Stop playback
      player.stop()
      expect(player.isPlaying()).toBe(false)
    }, 10000)

    test.skipIf(!testAudioFile)('should handle volume changes during playback', async () => {
      player.loadFile(testAudioFile)
      player.play()

      // Test volume changes while playing
      const volumes = [0.2, 0.5, 0.8, 1.0, 0.3]

      for (const volume of volumes) {
        player.setVolume(volume)
        expect(player.getVolume()).toBe(volume)
        await new Promise(resolve => setTimeout(resolve, 500))
      }

      player.stop()
    }, 10000)

    test.skipIf(!testAudioFile)('should handle rapid play/pause/stop operations', async () => {
      player.loadFile(testAudioFile)

      // Rapid state changes
      for (let i = 0; i < 5; i++) {
        player.play()
        expect(player.isPlaying()).toBe(true)

        await new Promise(resolve => setTimeout(resolve, 200))

        player.pause()
        expect(player.isPlaying()).toBe(false)

        await new Promise(resolve => setTimeout(resolve, 200))
      }

      player.stop()
    }, 10000)
  })

  describe('Multi-Instance Tests', () => {
    test.skipIf(!testAudioFile)('should handle multiple AudioPlayer instances', async () => {
      const players = Array.from({ length: 3 }, () => new AudioPlayer())

      // Load the same file in all players
      players.forEach(player => {
        expect(() => player.loadFile(testAudioFile)).not.toThrow()
      })

      // Start all players
      players.forEach(player => {
        player.play()
        expect(player.isPlaying()).toBe(true)
      })

      // Different volume settings for each
      players.forEach((player, index) => {
        player.setVolume(0.3 + (index * 0.2))
      })

      // Let them play
      await new Promise(resolve => setTimeout(resolve, 2000))

      // Stop all players
      players.forEach(player => {
        player.stop()
        expect(player.isPlaying()).toBe(false)
      })
    }, 10000)

    test.skipIf(!testAudioFile)('should handle createAudioPlayer helper', async () => {
      const options: PlaybackOptions = { volume: 0.7 }
      const customPlayer = createAudioPlayer(options)

      expect(customPlayer.getVolume()).toBe(0.7)
      expect(() => customPlayer.loadFile(testAudioFile)).not.toThrow()

      customPlayer.play()
      expect(customPlayer.isPlaying()).toBe(true)

      await new Promise(resolve => setTimeout(resolve, 1000))
      customPlayer.stop()
    }, 10000)
  })

  describe('Quick Play Functionality', () => {
    test.skipIf(!testAudioFile)('should play audio with quickPlay', async () => {
      const player = quickPlay(testAudioFile, { volume: 0.8, autoPlay: true })

      expect(player).toBeInstanceOf(AudioPlayer)
      expect(player.isPlaying()).toBe(true)
      expect(player.getVolume()).toBe(0.8)
      expect(player.currentFile).toBe(testAudioFile)

      await new Promise(resolve => setTimeout(resolve, 1000))
      player.stop()
    }, 10000)

    test.skipIf(!testAudioFile)('should create player without autoPlay', async () => {
      const player = quickPlay(testAudioFile, { autoPlay: false })

      expect(player).toBeInstanceOf(AudioPlayer)
      expect(player.isPlaying()).toBe(false)
      expect(player.currentFile).toBe(testAudioFile)

      player.play()
      expect(player.isPlaying()).toBe(true)

      await new Promise(resolve => setTimeout(resolve, 1000))
      player.stop()
    }, 10000)
  })

  describe('Format Detection', () => {
    test('should detect supported formats correctly', () => {
      const supportedFormats = getSupportedFormats()

      expect(isFormatSupported('wav')).toBe(true)
      expect(isFormatSupported('mp3')).toBe(true)
      expect(isFormatSupported('flac')).toBe(true)
      expect(isFormatSupported('ogg')).toBe(true)
      expect(isFormatSupported('unknown')).toBe(false)
      expect(isFormatSupported('')).toBe(false)
    })

    test.skipIf(!testAudioFile)('should detect format from file path', () => {
      const extension = testAudioFile.split('.').pop()?.toLowerCase()
      expect(extension).toBeTruthy()
      expect(isFormatSupported(extension!)).toBe(true)
    })
  })

  describe('Metadata Extraction', () => {
    test.skipIf(!testAudioFile)('should extract metadata from audio file', () => {
      const metadata = getAudioMetadata(testAudioFile)

      expect(typeof metadata).toBe('object')
      expect(metadata).toHaveProperty('format')

      const extension = testAudioFile.split('.').pop()?.toLowerCase()
      expect(metadata.format).toBe(extension)
    })
  })

  describe('Error Recovery', () => {
    test('should handle playback errors gracefully', () => {
      // Try to play non-existent file
      expect(() => player.loadFile('non-existent-file.mp3')).toThrow()

      // Player should still be functional
      expect(player.getVolume()).toBe(1.0)
      expect(player.isPlaying()).toBe(false)

      const devices = player.getDevices()
      expect(devices.length).toBeGreaterThan(0)
    })

    test.skipIf(!testAudioFile)('should recover from loading errors', async () => {
      // Try to load invalid file
      expect(() => player.loadFile('invalid.mp3')).toThrow()

      // Load valid file
      expect(() => player.loadFile(testAudioFile)).not.toThrow()

      // Should be able to play
      player.play()
      expect(player.isPlaying()).toBe(true)

      await new Promise(resolve => setTimeout(resolve, 500))
      player.stop()
    }, 10000)
  })

  describe('Performance Tests', () => {
    test('should handle rapid device queries', () => {
      const startTime = performance.now()

      for (let i = 0; i < 100; i++) {
        player.getDevices()
      }

      const endTime = performance.now()
      const duration = endTime - startTime

      // Should complete 100 queries in under 1 second
      expect(duration).toBeLessThan(1000)
      console.log(`100 device queries took ${duration.toFixed(2)}ms`)
    })

    test('should handle rapid volume changes', () => {
      const startTime = performance.now()

      for (let i = 0; i < 1000; i++) {
        player.setVolume(Math.random())
      }

      const endTime = performance.now()
      const duration = endTime - startTime

      // Should complete 1000 volume changes in under 100ms
      expect(duration).toBeLessThan(100)
      console.log(`1000 volume changes took ${duration.toFixed(2)}ms`)
    })

    test.skipIf(!testAudioFile)('should measure playback startup time', async () => {
      const iterations = 10
      const times: number[] = []

      for (let i = 0; i < iterations; i++) {
        const testPlayer = new AudioPlayer()

        const startTime = performance.now()
        testPlayer.loadFile(testAudioFile)
        testPlayer.play()
        const endTime = performance.now()

        times.push(endTime - startTime)

        // Wait a moment then stop
        await new Promise(resolve => setTimeout(resolve, 100))
        testPlayer.stop()
      }

      const avgTime = times.reduce((a, b) => a + b, 0) / times.length
      const maxTime = Math.max(...times)

      console.log(`Average startup time: ${avgTime.toFixed(2)}ms`)
      console.log(`Max startup time: ${maxTime.toFixed(2)}ms`)

      // Should start playback within 100ms on average
      expect(avgTime).toBeLessThan(100)
      expect(maxTime).toBeLessThan(500)
    }, 15000)
  })
})

describe('System Integration', () => {
  test('should initialize audio system consistently', () => {
    // Initialize multiple times
    for (let i = 0; i < 5; i++) {
      const result = initializeAudio()
      expect(typeof result).toBe('string')
      expect(result).toContain('initialized')
    }
  })

  test('should handle system audio device changes', () => {
    const player = new AudioPlayer()

    // Query devices multiple times
    const deviceLists = Array.from({ length: 5 }, () => player.getDevices())

    // All should return arrays
    deviceLists.forEach(devices => {
      expect(Array.isArray(devices)).toBe(true)
      expect(devices.length).toBeGreaterThan(0)
    })

    // Should return consistent results
    const firstDeviceList = deviceLists[0]
    deviceLists.forEach(devices => {
      expect(devices.length).toBe(firstDeviceList.length)
    })
  })

  test('should handle concurrent operations', async () => {
    const players = Array.from({ length: 5 }, () => new AudioPlayer())

    // Concurrent operations
    const promises = players.map(async (player, index) => {
      // Simulate different operations
      player.setVolume(0.1 + (index * 0.15))
      player.getDevices()
      player.getDuration()
      player.getCurrentTime()

      return player.getVolume()
    })

    const volumes = await Promise.all(promises)

    // Each should have different volume
    const uniqueVolumes = new Set(volumes)
    expect(uniqueVolumes.size).toBe(players.length)
  })
})
