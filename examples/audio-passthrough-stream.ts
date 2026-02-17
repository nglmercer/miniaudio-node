/**
 * Audio Passthrough Example - Using Native Streaming API
 * Real-time audio loopback (mic to speakers) with true streaming
 * 
 * Usage: bun examples/audio-passthrough-stream.ts [input-id] [output-id] [latency]
 * Use null for default devices
 * Example: bun examples/audio-passthrough-stream.ts null null 20
 */

// ==================== CONFIGURATION ====================
// Set to true to show available input/output devices on startup
const SHOW_DEVICES = false;
// ========================================================

import { AudioPassthrough } from "../index.js";
import {
  createLevelBar,
  formatDuration,
  colors,
  getLevelColor,
  clearLine,
} from "./utils/audio.js";

const POLL_INTERVAL_MS = 20;
const BAR_WIDTH = 5;

/**
 * Helper function to log devices in a clean way
 * Controlled by the SHOW_DEVICES constant at the top
 */
function logDevices(inputDevices: any[], outputDevices: any[]) {
  if (!SHOW_DEVICES) return;
  
  console.log(`${colors.bright}Available Input Devices:${colors.reset}`);
  inputDevices.forEach((dev, index) => {
    const defaultTag = dev.isDefault ? ` ${colors.yellow}[DEFAULT]${colors.reset}` : "";
    console.log(`  ${colors.green}${index + 1}.${colors.reset} ${dev.name}${defaultTag}`);
    console.log(`      ${colors.gray}ID: ${dev.id} | Host: ${dev.host}${colors.reset}`);
  });
  
  console.log(`\n${colors.bright}Available Output Devices:${colors.reset}`);
  outputDevices.forEach((dev, index) => {
    const defaultTag = dev.isDefault ? ` ${colors.yellow}[DEFAULT]${colors.reset}` : "";
    console.log(`  ${colors.green}${index + 1}.${colors.reset} ${dev.name}${defaultTag}`);
    console.log(`      ${colors.gray}ID: ${dev.id} | Host: ${dev.host}${colors.reset}`);
  });
  console.log();
}

async function main() {
  // Get available devices
  const inputDevices = AudioPassthrough.getInputDevices();
  const outputDevices = AudioPassthrough.getOutputDevices();
  
  // Show devices if enabled (controlled by SHOW_DEVICES constant)
  logDevices(inputDevices, outputDevices);
  
  console.log(`\n${colors.gray}Usage:${colors.reset}`);
  console.log(`  bun examples/audio-passthrough-stream.ts [input-id] [output-id] [latency-ms]`);
  
  // Parse args - use null for defaults
  let inputId: string | null = null;
  let outputId: string | null = null;
  let latencyMs = 20;
  
  const args = process.argv.slice(2);
  
  if (args[0] && args[0] !== "null") {
    inputId = args[0];
  }
  if (args[1] && args[1] !== "null") {
    outputId = args[1];
  }
  if (args[2]) {
    latencyMs = parseInt(args[2], 10) || 20;
  }
  const passthrough = new AudioPassthrough();
  console.log({
    outputId,
    inputId,
    latencyMs
  })
  
  try {
    passthrough.start(inputId, outputId, latencyMs);
  } catch (e) {
    console.log({e})
    process.exit(1);
  }

  console.log(`${colors.green}Passthrough running!${colors.reset}\n`);
  
  const startTime = Date.now();
  
  console.log(`${colors.gray}Time   | RMS (dB)  | Peak (dB) | Sample Rate | Channels | Level${colors.reset}`);
  console.log(`${colors.gray}${"─".repeat(80)}${colors.reset}`);

  while (passthrough.isRunning()) {
    const levels = passthrough.getLevels();
    const elapsed = (Date.now() - startTime) / 1000;
    
    const peakDb = levels.peak > 0 ? 20 * Math.log10(levels.peak) : -96;
    const rmsDb = levels.rms > 0 ? 20 * Math.log10(levels.rms) : -96;
    
    const levelBar = createLevelBar(rmsDb, BAR_WIDTH);
    const levelColor = getLevelColor(rmsDb);
    const timeStr = formatDuration(elapsed);
    
    clearLine();
    process.stdout.write(
      `${colors.cyan}${timeStr}${colors.reset} | ` +
      `${levelColor}${rmsDb.toFixed(1).padStart(8)} dB${colors.reset} | ` +
      `${levelColor}${peakDb.toFixed(1).padStart(8)} dB${colors.reset} | ` +
      `${passthrough.getSampleRate().toString().padStart(10)} Hz | ` +
      `${passthrough.getChannels().toString().padStart(2)} ch   | ` +
      levelBar
    );
    
    await new Promise(r => setTimeout(r, POLL_INTERVAL_MS));
  }
  
  clearLine();
  console.log(`\n${colors.green}Done!${colors.reset}\n`);
}

process.on('SIGINT', () => {
  console.log(`\n${colors.yellow}Stopping...${colors.reset}`);
  process.exit(0);
});

main().catch(console.error);
