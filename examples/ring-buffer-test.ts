import { getAvailableHosts, getInputDevicesByHost, AudioRecorder } from "../index.js";

const colors = {
  reset: "\x1b[0m",
  bright: "\x1b[1m",
  green: "\x1b[32m",
  cyan: "\x1b[36m",
  yellow: "\x1b[33m",
  red: "\x1b[31m",
};

async function main() {
  console.log(`${colors.bright}${colors.cyan}--- Audio Ring Buffer & Host Explorer ---${colors.reset}\n`);

  // 1. Show Available Hosts
  const hosts = getAvailableHosts();
  console.log(`${colors.bright}Available Audio Hosts:${colors.reset}`);
  hosts.forEach(h => {
    console.log(`  - ${colors.yellow}${h.id}${colors.reset}: ${h.name}`);
  });
  console.log("");

  // 2. Select first host (usually Alsa on Linux)
  const hostId = hosts[0].id;
  console.log(`Exploring host: ${colors.green}${hostId}${colors.reset}...`);
  const devices = getInputDevicesByHost(hostId);
  
  if (devices.length === 0) {
    console.log(`${colors.red}No devices found for this host.${colors.reset}`);
    return;
  }

  devices.forEach(d => {
    console.log(`  [${d.id}] ${d.name} ${d.isDefault ? colors.yellow + "(Default)" : ""}${colors.reset}`);
  });

  // 3. Setup Recorder with Ring Buffer
  const recorder = new AudioRecorder();
  
  // Set ring buffer size to 44100 samples (1 second of mono audio at 44.1k)
  const rbSize = 44100;
  console.log(`\nSetting up Ring Buffer of size: ${rbSize} samples...`);
  recorder.setRingBufferSize(rbSize);

  console.log(`Starting recording on default device...`);
  recorder.start();

  console.log(`${colors.yellow}Recording for 5 seconds. Polling ring buffer every 500ms...${colors.reset}\n`);

  const start = Date.now();
  while (Date.now() - start < 5000) {
    await new Promise(r => setTimeout(r, 500));
    
    // Poll the ring buffer
    const samples = recorder.getRingBufferSamples();
    if (samples.length > 0) {
      // Calculate simple peak for visualization
      let max = 0;
      for (const s of samples) {
        const abs = Math.abs(s);
        if (abs > max) max = abs;
      }
      const level = (max / 32768) * 50;
      const bar = "█".repeat(Math.round(level));
      console.log(`[RingBuffer] Got ${samples.length} samples. Peak: ${bar}`);
    }
  }

  recorder.stop();
  console.log(`\n${colors.green}Done!${colors.reset}`);
}

main().catch(console.error);
