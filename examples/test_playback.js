const {
  AudioPlayer,
  initializeAudio,
  getSupportedFormats,
} = require("../index.js");

async function testAudioPlayback() {
  try {
    // Initialize audio system
    console.log("Initializing audio system...");
    const initResult = initializeAudio();
    console.log("Init result:", initResult);

    // Create a new audio player instance
    console.log("\nCreating audio player...");
    const player = new AudioPlayer();

    // Get available audio devices
    console.log("\nAvailable audio devices:");
    const devices = player.getDevices();
    devices.forEach((device, index) => {
      console.log(
        `${index + 1}. ${device.name} (ID: ${device.id}, Default: ${device.is_default})`,
      );
    });

    // Get supported formats
    console.log("\nSupported audio formats:");
    const formats = getSupportedFormats();
    console.log(formats.join(", "));

    // Test with a Windows system sound
    console.log("\nLoading audio file: C:/Windows/Media/tada.wav");
    const audioFilePath = "C:/Windows/Media/tada.wav";

    try {
      player.loadFile(audioFilePath);
      console.log("✅ File loaded successfully!");

      // Get initial state
      console.log("\nInitial state:");
      console.log("Is playing:", player.isPlaying());
      console.log("Volume:", player.getVolume());

      // Start playback
      console.log("\n🎵 Starting playback...");
      player.play();

      // Check state after starting
      setTimeout(() => {
        console.log("After play() - Is playing:", player.isPlaying());
        console.log("Volume:", player.getVolume());
      }, 100);

      // Test volume control
      setTimeout(() => {
        console.log("\n🔊 Setting volume to 50%...");
        player.setVolume(0.5);
        console.log("New volume:", player.getVolume());
      }, 1000);

      // Test pause
      setTimeout(() => {
        console.log("\n⏸️ Pausing playback...");
        player.pause();
        console.log("Is playing:", player.isPlaying());
      }, 2000);

      // Test resume
      setTimeout(() => {
        console.log("\n▶️ Resuming playback...");
        player.play();
        console.log("Is playing:", player.isPlaying());
      }, 3000);

      // Test volume increase
      setTimeout(() => {
        console.log("\n🔊 Setting volume to 100%...");
        player.setVolume(1.0);
        console.log("New volume:", player.getVolume());
      }, 4000);

      // Test stop
      setTimeout(() => {
        console.log("\n⏹️ Stopping playback...");
        player.stop();
        console.log("Is playing:", player.isPlaying());
        console.log("Final volume:", player.getVolume());
      }, 5000);

      // Final state check
      setTimeout(() => {
        console.log("\n✅ Test completed successfully!");
        console.log("Duration (placeholder):", player.getDuration());
        console.log("Current time (placeholder):", player.getCurrentTime());
      }, 5500);
    } catch (loadError) {
      console.error("❌ Failed to load audio file:", loadError.message);
    }
  } catch (error) {
    console.error("❌ Error during test:", error.message);
  }
}

// Run the test
testAudioPlayback();

module.exports = {
  testAudioPlayback,
};
