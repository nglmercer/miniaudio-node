/**
 * Audio utility functions for WAV conversion and audio analysis
 * Reusable module for audio recording and monitoring applications
 */

import { writeFileSync } from "fs";

export interface AudioStats {
  peak: number;
  peakDb: number;
  rms: number;
  rmsDb: number;
  min: number;
  max: number;
  isSilent: boolean;
}

export interface WavOptions {
  sampleRate: number;
  channels: number;
  bitsPerSample?: number;
}

/**
 * ANSI color codes for terminal output
 */
export const colors = {
  reset: "\x1b[0m",
  bright: "\x1b[1m",
  green: "\x1b[32m",
  cyan: "\x1b[36m",
  yellow: "\x1b[33m",
  red: "\x1b[31m",
  magenta: "\x1b[35m",
  blue: "\x1b[34m",
  gray: "\x1b[90m",
};

/**
 * Analyzes audio samples and calculates audio statistics
 * @param samples - Array of audio samples (Int16Array or number array)
 * @returns AudioStats object with peak, RMS, and other metrics
 */
export function analyzeAudio(samples: number[] | Int16Array): AudioStats {
  const numSamples = samples.length;
  let max = 0;
  let min = 0;
  let sumSq = 0;

  for (let i = 0; i < numSamples; i++) {
    const s = samples[i];
    if (s > max) max = s;
    if (s < min) min = s;
    sumSq += s * s;
  }

  const rms = numSamples > 0 ? Math.sqrt(sumSq / numSamples) : 0;
  const peak = Math.max(Math.abs(max), Math.abs(min));
  
  // Convert to dB scale (relative to max possible 16-bit value)
  const peakDb = peak > 0 ? 20 * Math.log10(peak / 32768) : -96;
  const rmsDb = rms > 0 ? 20 * Math.log10(rms / 32768) : -96;

  return {
    peak,
    peakDb,
    rms,
    rmsDb,
    min,
    max,
    isSilent: max === 0 && min === 0,
  };
}

/**
 * Creates a visual audio level bar for terminal display
 * @param db - Level in dB (typically -96 to 0)
 * @param width - Width of the bar in characters
 * @returns Visual bar string with color coding
 */
export function createLevelBar(db: number, width: number = 20): string {
  // Normalize dB to 0-1 range (-60dB to 0dB range)
  const minDb = -60;
  const normalizedDb = Math.max(minDb, Math.min(0, db));
  const ratio = (normalizedDb - minDb) / (0 - minDb);
  
  const filled = Math.round(ratio * width);
  const empty = width - filled;
  
  let barColor = colors.green;
  if (db < -40) barColor = colors.gray;
  else if (db < -20) barColor = colors.cyan;
  else if (db < -6) barColor = colors.green;
  else if (db < 0) barColor = colors.yellow;
  else barColor = colors.red;
  
  const filledBar = "█".repeat(filled);
  const emptyBar = "░".repeat(empty);
  
  return `${barColor}${filledBar}${colors.gray}${emptyBar}${colors.reset}`;
}

/**
 * Converts PCM samples to WAV format buffer
 * @param samples - Audio samples (Int16Array or number array)
 * @param sampleRate - Sample rate in Hz
 * @param channels - Number of audio channels
 * @param bitsPerSample - Bits per sample (default: 16)
 * @returns Buffer containing WAV file data
 */
export function toWav(
  samples: number[] | Int16Array,
  sampleRate: number,
  channels: number,
  bitsPerSample: number = 16
): Buffer {
  const numSamples = samples.length;
  const bytesPerSample = bitsPerSample / 8;
  const dataSize = numSamples * bytesPerSample;
  const chunkSize = 36 + dataSize;

  const header = Buffer.alloc(44);
  
  // RIFF header
  header.write("RIFF", 0);
  header.writeUInt32LE(chunkSize, 4);
  header.write("WAVE", 8);

  // fmt chunk
  header.write("fmt ", 12);
  header.writeUInt32LE(16, 16); // subchunk1size
  header.writeUInt16LE(1, 20); // audio format (PCM)
  header.writeUInt16LE(channels, 22);
  header.writeUInt32LE(sampleRate, 24);
  header.writeUInt32LE(sampleRate * channels * bytesPerSample, 28); // byte rate
  header.writeUInt16LE(channels * bytesPerSample, 32); // block align
  header.writeUInt16LE(bitsPerSample, 34);

  // data chunk
  header.write("data", 36);
  header.writeUInt32LE(dataSize, 40);

  // Convert samples to Buffer with Little Endian
  const dataBuffer = Buffer.alloc(dataSize);
  for (let i = 0; i < numSamples; i++) {
    dataBuffer.writeInt16LE(samples[i], i * 2);
  }

  return Buffer.concat([header, dataBuffer]);
}

/**
 * Saves audio samples to a WAV file
 * @param samples - Audio samples
 * @param sampleRate - Sample rate in Hz
 * @param channels - Number of channels
 * @param filePath - Output file path
 * @returns AudioStats of the saved file
 */
export function saveWavFile(
  samples: number[] | Int16Array,
  sampleRate: number,
  channels: number,
  filePath: string
): AudioStats {
  const wavBuffer = toWav(samples, sampleRate, channels);
  writeFileSync(filePath, wavBuffer);
  
  return analyzeAudio(samples);
}

/**
 * Formats audio duration in human-readable format
 * @param seconds - Duration in seconds
 * @returns Formatted string (MM:SS)
 */
export function formatDuration(seconds: number): string {
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins.toString().padStart(2, "0")}:${secs.toString().padStart(2, "0")}`;
}

/**
 * Gets a color-coded string for audio level
 * @param db - Audio level in dB
 * @returns Color code for the level
 */
export function getLevelColor(db: number): string {
  if (db < -40) return colors.gray;
  if (db < -20) return colors.cyan;
  if (db < -6) return colors.green;
  if (db < 0) return colors.yellow;
  return colors.red;
}

/**
 * Clears the current line in terminal
 */
export function clearLine(): void {
  process.stdout.write("\r" + " ".repeat(process.stdout.columns) + "\r");
}
