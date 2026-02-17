import { getInputDevices, AudioRecorder } from "../index.js";
import { writeFileSync } from "fs";
import { toWav } from "./utils/audio.js";
// ANSI colors for better UI
const colors = {
  reset: "\x1b[0m",
  bright: "\x1b[1m",
  green: "\x1b[32m",
  cyan: "\x1b[36m",
  yellow: "\x1b[33m",
  red: "\x1b[31m",
};

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
