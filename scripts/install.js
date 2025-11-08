const { execSync } = require("child_process");
const path = require("path");

console.log("🔧 Building miniaudio-node module...");

try {
  execSync("napi build --platform --release", {
    stdio: "inherit",
    cwd: path.join(__dirname, ".."),
  });
  console.log("✅ miniaudio-node built successfully!");
} catch (error) {
  console.error("❌ Failed to build miniaudio-node:");
  console.error("   Run: npm run build");
  process.exit(1);
}
