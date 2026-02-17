/**
 * Audio Passthrough Example
 * Real-time audio monitoring: listen to microphone through speakers
 */

import { getInputDevices, AudioRecorder, AudioPlayer } from "../index.js";
import {
  createLevelBar,
  formatDuration,
  colors,
  getLevelColor,
  clearLine,
} from "./utils/audio.js";

// Configuration
const POLL_INTERVAL_MS = 50;
const DEFAULT_DURATION_SEC = 60;
const BAR_WIDTH = 20;
const CHUNK_SIZE = 4096;

/**
 * Displays device selection menu
 */
function displayDevices(devices: { id: string; name: string; isDefault: boolean; host: string }[]): void {
  console.log(`\n${colors.bright}${colors.cyan}=== Audio Passthrough - Device Selection ===${colors.reset}\n`);
  console.log(`${colors.bright}Available Input Devices:${colors.reset}`);
  
  devices.forEach((dev, index) => {
    const defaultTag = dev.isDefault ? ` ${colors.yellow}[DEFAULT]${colors.reset}` : "";
    console.log(`  ${colors.green}${index + 1}.${colors.reset} ${dev.name}${defaultTag}`);
    console.log(`      ${colors.gray}ID: ${dev.id} | Host: ${dev.host}${colors.reset}`);
  });
  
  console.log(`\n${colors.gray}Usage:${colors.reset}`);
  console.log(`  bun examples/audio-passthrough.ts [device-id] [duration-seconds]`);
  console.log(`  Example: bun examples/audio-passthrough.ts "Alsa:13" 60\n`);
  
  console.log(`${colors.yellow}⚠ WARNING: Keep mic away from speakers to avoid feedback loop!${colors.reset}\n`);
}

/**
 * Main function
 */
async function main() {
  console.log(`${colors.bright}${colors.cyan}╔════════════════════════════════════════╗${colors.reset}`);
  console.log(`${colors.bright}${colors.cyan}║     Audio Passthrough (Loopback)     ║${colors.reset}`);
  console.log(`${colors.bright}${colors.cyan}║     Listen to mic in real-time        ║${colors.reset}`);
  console.log(`${colors.bright}${colors.cyan}╚════════════════════════════════════════╝${colors.reset}\n`);

  const devices = getInputDevices();
  
  if (devices.length === 0) {
    console.error(`${colors.red}Error: No input devices found!${colors.reset}`);
    process.exit(1);
  }
  
  displayDevices(devices);

  let selectedDevice = devices.find((d) => d.isDefault) || devices[0];
  const deviceArg = process.argv[2];
  
  const inputDevices = devices.filter(d => 
    !d.name.toLowerCase().includes('discard') && 
    !d.name.toLowerCase().includes('null') &&
    !d.name.toLowerCase().includes('default output')
  );
  
  if (inputDevices.length > 0 && !deviceArg) {
    selectedDevice = inputDevices[0];
  }
  
  if (deviceArg) {
    const found = devices.find(
      (d) => d.id === deviceArg || d.id.endsWith(`:${deviceArg}`)
    );
    if (found) {
      selectedDevice = found;
    }
  }

  console.log(`${colors.green}Input device:${colors.reset} ${selectedDevice.name}`);
  console.log(`${colors.gray}Device ID:${colors.reset} ${selectedDevice.id}\n`);

  const durationArg = process.argv[3];
  let durationSec = DEFAULT_DURATION_SEC;
  
  if (durationArg) {
    durationSec = parseInt(durationArg, 10);
    if (isNaN(durationSec) || durationSec <= 0) {
      durationSec = DEFAULT_DURATION_SEC;
    }
  }

  // Create player
  console.log(`${colors.cyan}Initializing audio player...${colors.reset}`);
  const player = new AudioPlayer();
  player.setVolume(0.8);
  
  // Create recorder with ring buffer
  const recorder = new AudioRecorder();
  recorder.setRingBufferSize(44100); // 1 second buffer
  
  // Track samples
  let lastSampleCount = 0;
  let totalPlayed = 0;
  
  console.log(`${colors.yellow}Starting passthrough...${colors.reset}`);
  console.log(`${colors.yellow}Press Ctrl+C to stop${colors.reset}\n`);
  
  // Start recording
  recorder.start(selectedDevice.id);
  
  const startTime = Date.now();
  const endTime = startTime + durationSec * 1000;
  
  console.log(`${colors.gray}Time   | RMS (dB)  | Peak (dB) | Buffer   | Played${colors.reset}`);
  console.log(`${colors.gray}${"─".repeat(70)}${colors.reset}`);

  try {
    while (Date.now() < endTime) {
      // Get buffer samples
      const buffer = recorder.getBuffer();
      const allSamples = buffer.getSamples();
      
      // Get new samples since last check
      const newCount = allSamples.length - lastSampleCount;
      
      if (newCount >= CHUNK_SIZE) {
        // Get chunk of samples
        const chunk = allSamples.slice(lastSampleCount, lastSampleCount + CHUNK_SIZE);
        lastSampleCount += CHUNK_SIZE;
        
        // Play the chunk
        try {
          player.loadBuffer(chunk);
          player.play();
          totalPlayed += chunk.length;
        } catch (e) {
          // Ignore errors - player may be busy
        }
      }
      
      // Display levels from recorder
      const levels = recorder.getLevels();
      if (levels.peak > 0) {
        const elapsed = (Date.now() - startTime) / 1000;
        const timeStr = formatDuration(elapsed);
        
        const peakDb = levels.peak > 0 ? 20 * Math.log10(levels.peak) : -96;
        const rmsDb = levels.rms > 0 ? 20 * Math.log10(levels.rms) : -96;
        
        const levelBar = createLevelBar(rmsDb, BAR_WIDTH);
        const levelColor = getLevelColor(rmsDb);
        
        clearLine();
        process.stdout.write(
          `${colors.cyan}${timeStr}${colors.reset} | ` +
          `${levelColor}${rmsDb.toFixed(1).padStart(8)} dB${colors.reset} | ` +
          `${levelColor}${peakDb.toFixed(1).padStart(8)} dB${colors.reset} | ` +
          `${allSamples.length.toString().padStart(8)} | ` +
          `${totalPlayed.toString().padStart(6)} | ` +
          levelBar
        );
      }
      
      await new Promise((r) => setTimeout(r, POLL_INTERVAL_MS));
    }
  } catch (e) {
    console.error(`${colors.red}Error:${colors.reset}`, e);
  }
  
  clearLine();
  console.log(`\n${colors.green}Stopping...${colors.reset}`);
  
  recorder.stop();
  
  try {
    player.stop();
  } catch (e) {
    // Ignore stop errors
  }
  
  const finalBuffer = recorder.getBuffer();
  console.log(`\n${colors.bright}=== Summary ===${colors.reset}`);
  console.log(`Total buffer samples: ${colors.cyan}${finalBuffer.getSamples().length}${colors.reset}`);
  console.log(`Total samples played:  ${colors.cyan}${totalPlayed}${colors.reset}`);
  console.log(`\n${colors.green}Done!${colors.reset}\n`);
}

main().catch((err) => {
  console.error(`${colors.red}Fatal Error: ${err.message}${colors.reset}`);
  process.exit(1);
});
