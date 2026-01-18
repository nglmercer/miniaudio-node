#!/usr/bin/env bun
/**
 * Cross-platform artifact copy script using Bun shell
 * Compatible with Windows, Linux, and macOS
 */

import { $, Glob } from "bun";

const ARTIFACT_PATH = "dist";
const TARGET_ARTIFACT = process.argv[2] || "auto";

async function findNodeFiles() {
  console.log("🔍 Searching for .node files...");

  // Use glob pattern for cross-platform compatibility
  const files = await Array.fromAsync(new Glob("**/*.node").scan("."));

  // Filter out node_modules
  const filtered = files.filter((f) => !f.includes("node_modules"));

  console.log(`Found ${filtered.length} .node files`);
  return filtered;
}

async function detectArtifactName(nodeFile) {
  if (TARGET_ARTIFACT !== "auto") {
    return TARGET_ARTIFACT;
  }

  // Extract the pattern from the filename
  const filename = nodeFile.split("/").pop();
  console.log(`📁  Analyzing: ${filename}`);

  // Examples from napi-rs output:
  // miniaudio_node.darwin-x64.node
  // miniaudio_node.win32-x64-msvc.node
  // miniaudio_node.win32-ia32-msvc.node
  // miniaudio_node.linux-x64-gnu.node

  let artifact = "";

  if (filename.includes("win32")) {
    if (filename.includes("x64")) {
      artifact = "miniaudio_node.win32-x64-msvc.node";
    } else if (filename.includes("ia32")) {
      artifact = "miniaudio_node.win32-ia32-msvc.node";
    } else if (filename.includes("arm64")) {
      artifact = "miniaudio_node.win32-arm64-msvc.node";
    }
  } else if (filename.includes("darwin")) {
    if (filename.includes("arm64")) {
      artifact = "miniaudio_node.darwin-arm64.node";
    } else if (filename.includes("x64")) {
      artifact = "miniaudio_node.darwin-x64.node";
    } else {
      artifact = "miniaudio_node.darwin-universal.node";
    }
  } else if (filename.includes("linux")) {
    if (filename.includes("arm64") || filename.includes("aarch64")) {
      artifact = "miniaudio_node.linux-arm64-gnu.node";
    } else {
      artifact = "miniaudio_node.linux-x64-gnu.node";
    }
  }

  // Handle specific case for generic filename (e.g., from copy operation)
  if (!artifact && filename === "miniaudio_node.node") {
    console.log("⚠️  Generic filename detected, using platform detection...");
    const platform = process.platform;
    const arch = process.arch;

    if (platform === "win32") {
      if (arch === "x64") {
        artifact = "miniaudio_node.win32-x64-msvc.node";
      } else if (arch === "ia32") {
        artifact = "miniaudio_node.win32-ia32-msvc.node";
      } else if (arch === "arm64") {
        artifact = "miniaudio_node.win32-arm64-msvc.node";
      }
    } else if (platform === "darwin") {
      if (arch === "arm64") {
        artifact = "miniaudio_node.darwin-arm64.node";
      } else if (arch === "x64") {
        artifact = "miniaudio_node.darwin-x64.node";
      } else {
        artifact = "miniaudio_node.darwin-universal.node";
      }
    } else if (platform === "linux") {
      const isArm = arch === "arm64" || arch === "aarch64";
      artifact = isArm
        ? "miniaudio_node.linux-arm64-gnu.node"
        : "miniaudio_node.linux-x64-gnu.node";
    }
  }

  return artifact;
}

async function copyArtifact(source, destination) {
  console.log(`📦  Copying: ${source} -> ${destination}`);

  try {
    const dir = destination.substring(0, destination.lastIndexOf("/"));

    // Create directory (cross-platform)
    const mkdir = await $`mkdir -p ${dir}`.quiet();

    if (mkdir.exitCode !== 0) {
      throw new Error(`Failed to create directory: ${mkdir.text()}`);
    }

    // Copy file (cross-platform using Bun's fs)
    const sourcePath = source;
    const destPath = destination;

    const sourceExists = await Bun.file(sourcePath).exists();
    if (!sourceExists) {
      throw new Error(`Source file does not exist: ${sourcePath}`);
    }

    // Read source and write destination
    const sourceBuffer = await Bun.file(sourcePath).arrayBuffer();
    const destinationFile = Bun.file(destPath);
    await destinationFile.write(new Uint8Array(sourceBuffer));

    console.log(`✅ Copied successfully`);

    // Verify file size
    const destSize = Bun.file(destPath).size;
    console.log(`✅ File size: ${destSize} bytes`);

    return true;
  } catch (error) {
    console.error(`❌ Error copying: ${error.message}`);
    return false;
  }
}

async function main() {
  console.log("🚀 Starting cross-platform artifact copy...\n");

  // Ensure dist directory exists
  const mkdir = await $`mkdir -p ${ARTIFACT_PATH}`.quiet();
  if (mkdir.exitCode !== 0) {
    console.error("❌ Failed to create dist directory");
    process.exit(1);
  }

  const nodeFiles = await findNodeFiles();

  if (nodeFiles.length === 0) {
    console.error("❌ No .node files found!");
    console.error("This likely means the native build failed.");
    console.error("Check the build logs above for any compilation errors.");
    process.exit(1);
  }

  let copied = 0;
  let failed = 0;

  for (const nodeFile of nodeFiles) {
    const artifactName = await detectArtifactName(nodeFile);

    if (!artifactName) {
      console.error(`❌ Could not determine artifact name for: ${nodeFile}`);
      failed++;
      continue;
    }

    const destPath = `${ARTIFACT_PATH}/${artifactName}`;
    const success = await copyArtifact(nodeFile, destPath);

    if (success) {
      copied++;
    } else {
      failed++;
    }
  }

  console.log("\n" + "=".repeat(50));
  console.log("📋 Copy Summary");
  console.log("=".repeat(50));
  console.log(`✅ Copied: ${copied}`);
  console.log(`❌ Failed: ${failed}`);
  console.log(`📊 Total:  ${copied + failed}`);

  if (copied > 0) {
    console.log("\n📁 Files in dist/");
    const listCommand = $`ls -la ${ARTIFACT_PATH}`.quiet().catch(() => null);
    if (listCommand) {
      const listOutput = await listCommand;
      if (listOutput && listOutput.exitCode === 0) {
        console.log(listOutput.text());
      }
    }
  }

  if (failed > 0) {
    console.error("\n⚠️  Some artifacts failed to copy!");
    process.exit(1);
  }

  console.log("\n🎉 All artifacts copied successfully!");
}

// Run main
main().catch((error) => {
  console.error("💥 Fatal error:", error);
  process.exit(1);
});
