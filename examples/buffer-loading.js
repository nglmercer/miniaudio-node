/**
 * Example: Loading audio from buffers and base64 data
 * 
 * This example demonstrates how to load audio data from:
 * 1. Raw buffer data (Uint8Array)
 * 2. Base64 encoded audio data
 */

const { AudioPlayer, PlaybackState } = require('../index.js');

// Example 1: Load audio from a buffer
function loadFromBuffer() {
  console.log('=== Loading audio from buffer ===');
  
  const player = new AudioPlayer();
  
  // Create a minimal valid WAV file buffer (44 bytes header + 4 bytes data)
  // This is a very short silent audio clip
  const wavHeader = [
    0x52, 0x49, 0x46, 0x46, // "RIFF"
    0x24, 0x00, 0x00, 0x00, // File size - 8
    0x57, 0x41, 0x56, 0x45, // "WAVE"
    0x66, 0x6D, 0x74, 0x20, // "fmt "
    0x10, 0x00, 0x00, 0x00, // Format chunk size
    0x01, 0x00,             // Audio format (PCM)
    0x01, 0x00,             // Number of channels
    0x44, 0xAC, 0x00, 0x00, // Sample rate (44100)
    0x88, 0x58, 0x01, 0x00, // Byte rate
    0x02, 0x00,             // Block align
    0x10, 0x00,             // Bits per sample
    0x64, 0x61, 0x74, 0x61, // "data"
    0x04, 0x00, 0x00, 0x00, // Data chunk size
    0x00, 0x00, 0x00, 0x00  // 4 bytes of silence
  ];
  
  try {
    player.loadBuffer(wavHeader);
    console.log('✅ Buffer loaded successfully');
    console.log('State:', player.getState() === PlaybackState.Loaded ? 'Loaded' : 'Not loaded');
    console.log('Current file:', player.getCurrentFile());
    
    // You can now play the audio
    // player.play();
    
  } catch (error) {
    console.error('❌ Failed to load buffer:', error.message);
  }
}

// Example 2: Load audio from base64 data
function loadFromBase64() {
  console.log('\n=== Loading audio from base64 ===');
  
  const player = new AudioPlayer();
  
  // Base64 encoded minimal WAV file (same as above)
  const base64Wav = 'UklGRiQAAABXQVZFZm10IBAAAAABAAEAQB8AAEAfAAABAAgAZGF0YQQAAAAA';
  
  try {
    player.loadBase64(base64Wav);
    console.log('✅ Base64 audio loaded successfully');
    console.log('State:', player.getState() === PlaybackState.Loaded ? 'Loaded' : 'Not loaded');
    console.log('Current file:', player.getCurrentFile());
    
    // You can now play the audio
    // player.play();
    
  } catch (error) {
    console.error('❌ Failed to load base64:', error.message);
  }
}

// Example 3: Error handling
function demonstrateErrors() {
  console.log('\n=== Error handling examples ===');
  
  const player = new AudioPlayer();
  
  // Try to load empty buffer
  try {
    player.loadBuffer([]);
    console.log('❌ Should have thrown error for empty buffer');
  } catch (error) {
    console.log('✅ Correctly caught empty buffer error:', error.message);
  }
  
  // Try to load empty base64
  try {
    player.loadBase64('');
    console.log('❌ Should have thrown error for empty base64');
  } catch (error) {
    console.log('✅ Correctly caught empty base64 error:', error.message);
  }
  
  // Try to load invalid base64
  try {
    player.loadBase64('invalid-base64!');
    console.log('❌ Should have thrown error for invalid base64');
  } catch (error) {
    console.log('✅ Correctly caught invalid base64 error:', error.message);
  }
  
  // Try to load invalid audio data
  try {
    player.loadBase64('SGVsbG8gV29ybGQ='); // "Hello World" in base64
    console.log('❌ Should have thrown error for invalid audio data');
  } catch (error) {
    console.log('✅ Correctly caught invalid audio data error:', error.message);
  }
}

// Run all examples
function runAllExamples() {
  console.log('🎵 Audio Buffer Loading Examples\n');
  
  try {
    loadFromBuffer();
    loadFromBase64();
    demonstrateErrors();
    
    console.log('\n✅ All examples completed successfully!');
  } catch (error) {
    console.error('❌ Example failed:', error);
  }
}

// Run examples if this file is executed directly
if (require.main === module) {
  runAllExamples();
}

module.exports = {
  loadFromBuffer,
  loadFromBase64,
  demonstrateErrors
};