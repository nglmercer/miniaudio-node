import { getInputDevices, AudioRecorder } from "../index.js";
import { writeFileSync } from "fs";

// ANSI colors for better UI
const colors = {
  reset: "\x1b[0m",
  bright: "\x1b[1m",
  green: "\x1b[32m",
  cyan: "\x1b[36m",
  yellow: "\x1b[33m",
  red: "\x1b[31m",
};

/**
 * Saves PCM samples to a WAV file
 */
function toWav(samples: number[] | Int16Array, sampleRate: number, channels: number) {
  const bitsPerSample = 16;
  const numSamples = samples.length;
  const dataSize = numSamples * 2;
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
  header.writeUInt32LE(sampleRate * channels * 2, 28); // byte rate
  header.writeUInt16LE(channels * 2, 32); // block align
  header.writeUInt16LE(bitsPerSample, 34);

  // data chunk
  header.write("data", 36);
  header.writeUInt32LE(dataSize, 40);

  // Convert samples to Buffer with explicit Little Endian if needed
  // Using a DataView or writing to Buffer directly is safest
  const dataBuffer = Buffer.alloc(dataSize);
  for (let i = 0; i < numSamples; i++) {
    dataBuffer.writeInt16LE(samples[i], i * 2);
  }

  const finalBuffer = Buffer.concat([header, dataBuffer]);
  // Audio quality check
  let max = 0;
  let min = 0;
  let sumSq = 0;
  for (let i = 0; i < numSamples; i++) {
    const s = samples[i];
    if (s > max) max = s;
    if (s < min) min = s;
    sumSq += s * s;
  }
  const rms = Math.sqrt(sumSq / numSamples);
  const peakDb = 20 * Math.log10(Math.max(Math.abs(max), Math.abs(min)) / 32768);

  console.log(`${colors.yellow}Audio Stats: Peak: ${peakDb.toFixed(1)} dB, RMS: ${(20 * Math.log10(rms / 32768)).toFixed(1)} dB${colors.reset}`);
  
  if (max === 0 && min === 0) {
    console.warn(`${colors.red}WARNING: The recording is total SILENCE (all zeros)!${colors.reset}`);
  }
  return finalBuffer
}

async function main() {
  console.log(`${colors.bright}${colors.cyan}--- Audio Recorder Console ---${colors.reset}\n`);

  const devices = getInputDevices();
  console.log(`${colors.bright}Available Input Devices:${colors.reset}`);
  devices.forEach((dev) => {
    const defaultTag = dev.isDefault ? ` ${colors.yellow}[DEFAULT]${colors.reset}` : "";
    console.log(`  ${colors.green}[${dev.id}]${colors.reset} ${dev.name}${defaultTag}`);
  });
  console.log("");

  const recorder = new AudioRecorder();
  
  // Get device ID from command line argument
  const selectedDeviceId = process.argv[2];
  
  if (selectedDeviceId !== undefined) {
    // Try to find by ID (new format Host:Index or old format Index)
    const device = devices.find(d => d.id === selectedDeviceId || d.id.endsWith(`:${selectedDeviceId}`));
    if (!device) {
      console.error(`${colors.red}Error: Device with ID "${selectedDeviceId}" not found.${colors.reset}`);
      process.exit(1);
    }
    console.log(`Recording from: ${colors.green}${device.name}${colors.reset} [${device.host}]`);
    recorder.start(device.id); // Use the full ID
  } else {
    console.log(`${colors.yellow}No device ID provided. Using default device.${colors.reset}`);
    console.log(`${colors.cyan}Tip: Run "bun examples/record-test.ts [id]" to pick a specific device.${colors.reset}\n`);
    recorder.start();
  }

  const durationSec = 5;
  console.log(`Starting ${durationSec}s recording...`);
  
  // Progress bar animation
  const startTime = Date.now();
  const endTime = startTime + (durationSec * 1000);
  
  while (Date.now() < endTime) {
    await new Promise(r => setTimeout(r, 100));
  }
  process.stdout.write("completed");

  recorder.stop();
  console.log(`\n${colors.bright}Recording stopped.${colors.reset}`);

  const buffer = recorder.getBuffer();
  const samples = buffer.getSamples();
  const sampleRate = buffer.getSampleRate();
  const channels = buffer.getChannels();
  const duration = buffer.getDuration();

  console.log(`\n${colors.bright}Recording Info:${colors.reset}`);
  console.log(`  - Format: ${colors.cyan}PCM 16-bit${colors.reset}`);
  console.log(`  - Sample Rate: ${colors.cyan}${sampleRate} Hz${colors.reset}`);
  console.log(`  - Channels: ${colors.cyan}${channels}${colors.reset}`);
  console.log(`  - Duration: ${colors.cyan}${duration.toFixed(2)} seconds${colors.reset}`);
  console.log(`  - Total Samples: ${colors.cyan}${samples.length}${colors.reset}`);

  if (samples.length > 0) {
    const buffer = toWav(samples, sampleRate, channels);
    writeFileSync("recording.wav", buffer);

    console.log(`\n${colors.bright}${colors.green}SUCCESS: File "recording.wav" is ready!${colors.reset}\n`);
  } else {
    console.log(`\n${colors.red}ERROR: No samples were captured. Check your microphone settings.${colors.reset}\n`);
  }
}

main().catch(err => {
  console.error(`\n${colors.red}Fatal Error: ${err.message}${colors.reset}`);
});
