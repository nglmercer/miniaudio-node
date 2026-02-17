/**
 * Audio Monitor with Real-time Playback
 * 
 * Records from microphone and plays back in real-time.
 * Perfect for testing audio input/output latency.
 * 
 * Usage:
 *   bun examples/audio-monitor.ts [device-id]
 *   
 * Example:
 *   bun examples/audio-monitor.ts "Alsa:13"
 * 
 * Press Ctrl+C to exit
 */

import { getInputDevices, AudioRecorder, AudioPlayer, PlaybackState } from "../index.js";
import {
  analyzeAudio,
  createLevelBar,
  formatDuration,
  colors,
  getLevelColor,
  clearLine,
} from "./utils/audio.js";

// Configuration
const POLL_INTERVAL_MS = 80;
const BAR_WIDTH = 25;
const CHUNK_SIZE_MS = 100; // Audio chunk size in milliseconds

/**
 * Displays device selection menu
 */
function displayDevices(devices: { id: string; name: string; isDefault: boolean; host: string }[]): void {
  console.log(`\n${colors.bright}${colors.cyan}=== Audio Monitor - Device Selection ===${colors.reset}\n`);
  console.log(`${colors.bright}Available Input Devices:${colors.reset}`);
  
  devices.forEach((dev, index) => {
    const defaultTag = dev.isDefault ? ` ${colors.yellow}[DEFAULT]${colors.reset}` : "";
    console.log(`  ${colors.green}${index + 1}.${colors.reset} ${dev.name}${defaultTag}`);
    console.log(`      ${colors.gray}ID: ${dev.id} | Host: ${dev.host}${colors.reset}`);
  });
  
  console.log(`\n${colors.gray}Usage:${colors.reset}`);
  console.log(`  bun examples/audio-monitor.ts [device-id]`);
  console.log(`  Example: bun examples/audio-monitor.ts "Alsa:13"\n`);
}

/**
 * Real-time audio monitoring with playback
 */
async function monitorWithPlayback(recorder: AudioRecorder, player: AudioPlayer): Promise<void> {
  const startTime = Date.now();
  let lastSampleCount = 0;
  let chunkStartCount = 0;
  
  // Get initial sample rate
  const sampleRate = recorder.getBuffer().getSampleRate() || 44100;
  const channels = recorder.getBuffer().getChannels() || 1;
  const samplesPerChunk = Math.floor(sampleRate * CHUNK_SIZE_MS / 1000) * channels;
  
  console.log(`\n${colors.bright}${colors.cyan}=== Real-time Audio Monitor + Playback ===${colors.reset}`);
  console.log(`Sample Rate: ${sampleRate} Hz | Channels: ${channels}`);
  console.log(`Chunk Size: ${CHUNK_SIZE_MS}ms (${samplesPerChunk} samples)`);
  console.log(`Press ${colors.yellow}Ctrl+C${colors.reset} to exit\n`);
  
  // Print header
  console.log(`${colors.gray}Time   | RMS (dB)  | Peak (dB) | Visual${colors.reset}`);
  console.log(`${colors.gray}${"─".repeat(55)}${colors.reset}`);

  while (recorder.isRecording()) {
    try {
      const buffer = recorder.getBuffer();
      const samples = buffer.getSamples();
      
      // Get new samples since last check
      const newSamples = samples.length > lastSampleCount 
        ? samples.slice(lastSampleCount) 
        : [];
      lastSampleCount = samples.length;
      
      // Calculate how many samples we need for playback
      const pendingSamples = samples.length - chunkStartCount;
      
      // When we have enough samples, play them
      if (pendingSamples >= samplesPerChunk) {
        const chunk = samples.slice(chunkStartCount, chunkStartCount + samplesPerChunk);
        chunkStartCount += samplesPerChunk;
        
        // Convert to number array and play
        const chunkArray = Array.from(chunk);
        
        // Check if player is ready
        const state = player.getState();
        if (state === PlaybackState.Stopped || state === PlaybackState.Paused) {
          try {
            player.loadBuffer(chunkArray);
            player.play();
          } catch (e) {
            // Ignore playback errors
          }
        }
      }
      
      // Display visualization for the latest chunk
      if (newSamples.length > 0 && newSamples.length < 50000) {
        const stats = analyzeAudio(newSamples);
        
        const elapsed = (Date.now() - startTime) / 1000;
        const timeStr = formatDuration(elapsed);
        const levelBar = createLevelBar(stats.rmsDb, BAR_WIDTH);
        
        const rmsStr = `${stats.rmsDb.toFixed(1)} dB`;
        const peakStr = `${stats.peakDb.toFixed(1)} dB`;
        const levelColor = getLevelColor(stats.rmsDb);
        const peakColor = getLevelColor(stats.peakDb);
        
        // Playback status indicator
        const state = player.getState();
        let statusIcon = `${colors.gray}○${colors.reset}`;
        if (state === PlaybackState.Playing) {
          statusIcon = `${colors.green}▶${colors.reset}`;
        } else if (state === PlaybackState.Paused) {
          statusIcon = `${colors.yellow}⏸${colors.reset}`;
        }
        
        clearLine();
        process.stdout.write(
          `${statusIcon} ${colors.cyan}${timeStr}${colors.reset} | ` +
          `${levelColor}${rmsStr.padStart(9)}${colors.reset} | ` +
          `${peakColor}${peakStr.padStart(10)}${colors.reset} | ` +
          levelBar
        );
      }
    } catch (e) {
      // Continue on error
    }
    
    await new Promise((r) => setTimeout(r, POLL_INTERVAL_MS));
  }
  
  clearLine();
  console.log(`\n${colors.yellow}Monitoring stopped.${colors.reset}`);
}

/**
 * Main function
 */
async function main() {
  console.log(`${colors.bright}${colors.cyan}╔════════════════════════════════════════╗${colors.reset}`);
  console.log(`${colors.bright}${colors.cyan}║  Audio Monitor + Real-time Playback  ║${colors.reset}`);
  console.log(`${colors.bright}${colors.cyan}╚════════════════════════════════════════╝${colors.reset}\n`);

  const devices = getInputDevices();
  
  if (devices.length === 0) {
    console.error(`${colors.red}Error: No input devices found!${colors.reset}`);
    process.exit(1);
  }
  
  displayDevices(devices);

  // Filter out null/devnull devices, sound servers, and OUTPUT devices
  const inputDevices = devices.filter(d => {
    const name = d.name.toLowerCase();
    return !name.includes('discard') && 
           !name.includes('null') &&
           !name.includes('default output') &&
           !name.includes('sound server') &&
           !name.includes('pipewire') && // These are often virtual devices
           !name.includes('pulseaudio');
  });
  
  let selectedDevice = inputDevices.length > 0 ? inputDevices[0] : devices[0];
  
  const deviceArg = process.argv[2];
  
  if (deviceArg) {
    const found = devices.find(
      (d) => d.id === deviceArg || d.id.endsWith(`:${deviceArg}`)
    );
    if (found) {
      selectedDevice = found;
    } else {
      console.warn(
        `${colors.yellow}Warning: Device "${deviceArg}" not found. Using default.${colors.reset}`
      );
    }
  }

  console.log(`${colors.green}Input device:${colors.reset} ${selectedDevice.name}`);
  console.log(`${colors.gray}Device ID:${colors.reset} ${selectedDevice.id}\n`);

  // Create recorder and player
  const recorder = new AudioRecorder();
  const player = new AudioPlayer();
  
  // Set volume to 70%
  player.setVolume(0.7);
  
  console.log(`${colors.cyan}Starting audio monitor with playback...${colors.reset}`);
  console.log(`${colors.gray}Volume: 70%${colors.reset}\n`);
  
  // Start recording
  recorder.start(selectedDevice.id);
  
  // Wait a moment for the recorder to initialize
  await new Promise((r) => setTimeout(r, 200));
  
  // Handle Ctrl+C
  const handleShutdown = () => {
    console.log(`\n\n${colors.yellow}Stopping monitor...${colors.reset}`);
    recorder.stop();
    player.stop();
    process.exit(0);
  };
  
  process.on('SIGINT', handleShutdown);
  process.on('SIGTERM', handleShutdown);
  
  await monitorWithPlayback(recorder, player);
  
  recorder.stop();
  player.stop();
}

main().catch((err) => {
  console.error(`\n${colors.red}Fatal Error: ${err.message}${colors.reset}`);
  process.exit(1);
});
