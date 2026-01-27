/**
 * Playback Controls - Pause, Resume, Seek demo
 */

import { AudioPlayer, AudioDecoder, setDebug } from "../index.js";
/*
const initResult = initializeAudio();
console.log({initResult});
*/
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
player.loadFile(file);
//player.loadBase64("");
//player.loadBuffer([]);
player.play();

console.log("▶️ Playing...");
//@ts-ignore
const monitorInterval = setInterval(() => {
    console.log(
      `   Current time: ${player.getCurrentTime().toFixed(1)}s / ${player.getDuration().toFixed(1)}s`,player.getState()
    );
}, 1000);
// Seek demo after 2 more seconds
setTimeout(async () => {
  console.log("\n🔍 Seeking to middle...");
  player.seekTo(20);
}, 5000);
