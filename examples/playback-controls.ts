/**
 * Playback Controls - Pause, Resume, Seek demo
 */

import { AudioPlayer, AudioDecoder, SamplesBuffer, setDebug } from "../index.js";
setDebug(true)
const file = process.argv[2] || "/home/meme/Música/test.mp3";
const player = new AudioPlayer();
const decoder = new AudioDecoder(file);
const duration = decoder.getDuration();

console.log("File:", file);
console.log("Duration:", duration.toFixed(1), "s");
console.log("Volume:", player.getVolume());

// Set volume to 100%
player.setVolume(1.0);
console.log("Volume set to:", player.getVolume());

// Load and play
await player.loadFile(file);
player.play();

console.log("▶️ Playing...");

// Keep process alive until playback finishes
const keepAlive = setInterval(() => {
  if (!player.isPlaying()) {
    clearInterval(keepAlive);
  }
}, 100);


// Seek demo after 2 more seconds
setTimeout(async () => {
  console.log("\n🔍 Seeking to middle...");
  player.stop();
  
  const pos = duration / 2;
  const samples = decoder.decodeSlice(pos, Math.min(pos + 10, duration));
  const buf = new SamplesBuffer(decoder.getChannels(), decoder.getSampleRate(), samples);
  
  buf.play();
  console.log("▶️ Playing from", pos.toFixed(0), "s (10s chunk)");
}, 5000);
