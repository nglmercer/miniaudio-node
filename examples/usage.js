const {
  AudioPlayer,
  initializeAudio,
  getSupportedFormats,
} = require("../index.js");

async function demonstrateAudio() {
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

    // Load an audio file (you'll need to provide a valid path)
    console.log("\nLoading audio file...");
    // const audioFilePath = 'path/to/your/audio/file.mp3';
    // player.loadFile(audioFilePath);
    console.log(
      "Note: Uncomment and provide a valid audio file path to test playback",
    );

    // Example of playback controls (uncomment when you have a file loaded)
    /*
        console.log('\nStarting playback...');
        player.play();

        // Wait a bit to demonstrate playback
        setTimeout(() => {
            console.log('Current time:', player.getCurrentTime());
            console.log('Duration:', player.getDuration());
            console.log('Is playing:', player.isPlaying());
        }, 2000);

        setTimeout(() => {
            console.log('\nSetting volume to 0.5');
            player.setVolume(0.5);
        }, 3000);

        setTimeout(() => {
            console.log('\nPausing playback...');
            player.pause();
        }, 5000);

        setTimeout(() => {
            console.log('\nResuming playback...');
            player.play();
        }, 7000);

        setTimeout(() => {
            console.log('\nStopping playback...');
            player.stop();
            console.log('Final volume:', player.getVolume());
        }, 10000);
        */
  } catch (error) {
    console.error("Error:", error);
  }
}

// Run the demonstration
demonstrateAudio();

module.exports = {
  demonstrateAudio,
};
