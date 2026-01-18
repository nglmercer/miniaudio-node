#!/usr/bin/env bun
/**
 * Cross-platform artifact copy script using Bun shell
 * Compatible with Windows, Linux, and macOS
 */

import { $ } from "bun";

const ARTIFACT_PATH = "dist";
const TARGET_ARTIFACT = process.argv[2] || "auto";

async function findNodeFiles() {
  console.log("🔍 Searching for .node files...");

  // Bun shell supports cross-platform patterns
  const result = await $`find . -maxdepth 2 -name "*.node"`.quiet();
  const output = result.text();

  const files = output
    .split("\n")
    .filter((line) => line.trim() && !line.includes("node_modules"));

  console.log(`Found ${files.length} .node files`);
  return files;
}

async function detectArtifactName(nodeFile) {
  if (TARGET_ARTIFACT !== "auto") {
    return TARGET_ARTIFACT;
  }

  // Extract the pattern from the filename
  const filename = nodeFile.split("/").pop();
  console.log(`📁  Analyzing: ${filename}`);

  // Examples:
  // miniaudio_node.win32-x64-msvc.node
  // miniaudio_node.darwin-x64.node
  // miniaudio_node.linux-x64-gnu.node

  let artifact = "";

  if (filename.includes("win32")) {
    if (filename.includes("x64")) {
      artifact = "miniaudio_node.win32-x64-msvc.node";
    } else if (filename.includes("ia32")) {
      artifact = "miniaudio_node.win32-ia32-msvc.node";
    }
  } else if (filename.includes("darwin")) {
    artifact = filename.includes("arm64")
      ? "miniaudio_node.darwin-arm64.node"
      : "miniaudio_node.darwin-x64.node";
  } else if (filename.includes("linux")) {
    if (filename.includes("arm64")) {
      artifact = "miniaudio_node.linux-arm64-gnu.node";
    } else {
      artifact = "miniaudio_node.linux-x64-gnu.node";
    }
  }

  if (!artifact && filename === "miniaudio_node.node") {
    // Generic filename - we need to detect from environment
    console.log("⚠️  Generic filename detected, using platform detection...");
    const platform = process.platform;
    const arch = process.arch;

    if (platform === "win32") {
      artifact =
        arch === "x64"
          ? "miniaudio_node.win32-x64-msvc.node"
          : "miniaudio_node.win32-ia32-msvc.node";
    } else if (platform === "darwin") {
      artifact = `miniaudio_node.darwin-${arch}.node`;
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

    // Copy file (cross-platform)
    const cp = await $`cp ${source} ${destination}`.quiet();

    if (cp.exitCode !== 0) {
      throw new Error(`Failed to copy file: ${cp.text()}`);
    }

    console.log(`✅ Copied successfully`);

    // Verify file size
    const size = await $`stat -c%s ${destination}`.quiet();

    // Cross-platform fallback
    if (size.exitCode !== 0) {
      const altSize = await $`stat -f%z ${destination}`.quiet();
      if (altSize.exitCode === 0) {
        console.log(`✅ File size: ${altSize.text().trim()} bytes`);
      }
    } else {
      console.log(`✅ File size: ${size.text().trim()} bytes`);
    }

    return true;
  } catch (error) {
    console.error(`❌ Error copying: ${error.message}`);
    return false;
  }
}

async function main() {
  console.log("🚀 Starting cross-platform artifact copy...\n");

  // Ensure dist directory exists
  await $`mkdir -p ${ARTIFACT_PATH}`.quiet();

  const nodeFiles = await findNodeFiles();

  if (nodeFiles.length === 0) {
    console.error("❌ No .node files found!");
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
    await $`ls -la ${ARTIFACT_PATH}`.quiet().catch(() => {});
  }

  if (failed > 0) {
    process.exit(1);
  }

  console.log("\n🎉 All artifacts copied successfully!");
}

// Run main
main().catch((error) => {
  console.error("💥 Fatal error:", error);
  process.exit(1);
});
