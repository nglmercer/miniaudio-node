/**
 * Test Utilities for Multiplatform Audio Testing
 *
 * This module provides cross-platform utilities for audio testing,
 * including platform-specific test file paths and audio system helpers.
 */

import * as path from 'path'
import * as fs from 'fs'

export interface PlatformInfo {
  platform: string
  isWindows: boolean
  isMacOS: boolean
  isLinux: boolean
}

export const PLATFORM: PlatformInfo = {
  platform: process.platform,
  isWindows: process.platform === 'win32',
  isMacOS: process.platform === 'darwin',
  isLinux: process.platform === 'linux'
}

/**
 * Get platform-specific test audio file paths
 */
export function getTestAudioFiles(): string[] {
  const files: string[] = []

  if (PLATFORM.isWindows) {
    // Windows system sounds
    const windowsMedia = 'C:/Windows/Media'
    if (fs.existsSync(windowsMedia)) {
      files.push(
        path.join(windowsMedia, 'tada.wav'),
        path.join(windowsMedia, 'chimes.wav'),
        path.join(windowsMedia, 'notify.wav')
      )
    }
  } else if (PLATFORM.isMacOS) {
    // macOS system sounds
    const macSounds = '/System/Library/Sounds'
    if (fs.existsSync(macSounds)) {
      files.push(
        path.join(macSounds, 'Glass.aiff'),
        path.join(macSounds, 'Pop.aiff'),
        path.join(macSounds, 'Tink.aiff')
      )
    }
  } else if (PLATFORM.isLinux) {
    // Linux system sounds (common locations)
    const linuxSoundPaths = [
      '/usr/share/sounds/alsa/Front_Left.wav',
      '/usr/share/sounds/alsa/Front_Right.wav',
      '/usr/share/sounds/ubuntu/stereo/bell.ogg'
    ]
    
    linuxSoundPaths.forEach(soundPath => {
      if (fs.existsSync(soundPath)) {
        files.push(soundPath)
      }
    })
  }

  // Filter to only existing files
  return files.filter(file => fs.existsSync(file))
}

/**
 * Get the first available test audio file
 */
export function getFirstTestAudioFile(): string | null {
  const files = getTestAudioFiles()
  return files.length > 0 ? files[0] : null
}

/**
 * Safe audio system initialization with error handling
 */
export function safeInitializeAudio(): string | null {
  try {
    const { initializeAudio } = require('../../index.js')
    const result = initializeAudio()
    return result
  } catch (error) {
    console.warn('Audio system initialization failed:', error)
    return null
  }
}

/**
 * Check if audio system is available
 */
export function isAudioSystemAvailable(): boolean {
  try {
    const { initializeAudio, getSupportedFormats } = require('../../index.js')
    initializeAudio()
    const formats = getSupportedFormats()
    return Array.isArray(formats) && formats.length > 0
  } catch (error) {
    return false
  }
}

/**
 * Skip test if audio system is not available
 */
export function skipIfNoAudio(): void {
  if (!isAudioSystemAvailable()) {
    console.warn('Skipping test: Audio system not available')
    // In a real test framework, this would skip the test
    // For Bun, we'll just log and continue
  }
}

/**
 * Get platform-specific timeout for audio operations
 */
export function getAudioTestTimeout(): number {
  // Windows may need more time for audio operations
  return PLATFORM.isWindows ? 10000 : 5000
}

/**
 * Common test configuration
 */
export const TEST_CONFIG = {
  timeout: getAudioTestTimeout(),
  retryAttempts: 3,
  retryDelay: 1000
}

/**
 * Validate audio file existence and format
 */
export function validateAudioFile(filePath: string): boolean {
  if (!fs.existsSync(filePath)) {
    return false
  }

  const ext = path.extname(filePath).toLowerCase()
  const supportedExts = ['.wav', '.mp3', '.flac', '.ogg', '.aiff', '.aif']
  return supportedExts.includes(ext)
}

/**
 * Get test file path with validation
 */
export function getValidTestFilePath(): string | null {
  const files = getTestAudioFiles()
  const validFiles = files.filter(validateAudioFile)
  return validFiles.length > 0 ? validFiles[0] : null
}
