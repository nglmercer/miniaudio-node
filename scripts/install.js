const { execSync } = require("child_process");
const path = require("path");

console.log("🔧 Building native-audio-playback module...");

try {
  execSync("napi build --platform --release", {
    stdio: "inherit",
    cwd: path.join(__dirname, ".."),
  });
  console.log("✅ native-audio-playback built successfully!");
} catch (error) {
  console.error("❌ Failed to build native-audio-playback:");
  console.error("   Run: npm run build");
  process.exit(1);
}
