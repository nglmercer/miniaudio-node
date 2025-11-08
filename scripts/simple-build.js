#!/usr/bin/env node
/**
 * Simple Build Script for MiniAudio Node
 *
 * This script builds the project step by step with detailed explanations
 * to help understand how the pieces fit together.
 */

const { execSync } = require('child_process');
const { existsSync, mkdirSync, copyFileSync } = require('fs');
const { join } = require('path');

console.log('🚀 MiniAudio Node - Simple Build Process');
console.log('='.repeat(50));

// Step 1: Build TypeScript to dist/
console.log('\n📝 Step 1: Building TypeScript...');
try {
  // Ensure dist directory exists
  if (!existsSync('dist')) {
    mkdirSync('dist', { recursive: true });
    console.log('✅ Created dist/ directory');
  }

  // Compile TypeScript with Bun
  execSync('bun build src/index.ts --target node --outdir dist --minify --sourcemap', { stdio: 'inherit' });
  console.log('✅ TypeScript compiled to dist/index.js');
} catch (error) {
  console.error('❌ TypeScript build failed:', error.message);
  process.exit(1);
}

// Step 2: Build native module
console.log('\n🦀 Step 2: Building native Rust module...');
try {
  execSync('cd native && cargo build --release', { stdio: 'inherit' });
  console.log('✅ Native module built in native/target/release/');
} catch (error) {
  console.error('❌ Native build failed:', error.message);
  console.log('\n💡 Make sure Rust is installed: https://rustup.rs/');
  process.exit(1);
}

// Step 3: Copy native binary to dist/
console.log('\n📦 Step 3: Copying native binary...');
try {
  // Determine platform-specific binary name
  const platform = process.platform;
  const arch = process.arch;

  let binaryName;
  switch (platform) {
    case 'win32':
      binaryName = `miniaudio-node.win32-${arch}-msvc.node`;
      break;
    case 'darwin':
      binaryName = `miniaudio-node.darwin-${arch}.node`;
      break;
    case 'linux':
      binaryName = `miniaudio-node.linux-${arch}-gnu.node`;
      break;
    default:
      throw new Error(`Unsupported platform: ${platform}`);
  }

  // Copy from Rust target to dist
  const sourcePath = join('native', 'target', 'release', 'miniaudio_ffi' + (platform === 'win32' ? '.dll' : '.so'));
  const destPath = join('dist', binaryName);

  // For Windows, look for .node file directly
  if (platform === 'win32') {
    const winSource = join('native', 'target', 'release', 'miniaudio_ffi.dll');
    if (existsSync(winSource)) {
      copyFileSync(winSource, destPath);
      console.log(`✅ Copied ${binaryName} to dist/`);
    } else {
      throw new Error('Native binary not found. Check if build completed successfully.');
    }
  } else {
    copyFileSync(sourcePath, destPath);
    console.log(`✅ Copied ${binaryName} to dist/`);
  }

  // Copy the existing .node file if it exists (fallback)
  const existingNode = join('dist', 'native-audio-playback.win32-x64-msvc.node');
  if (existsSync(existingNode)) {
    copyFileSync(existingNode, join('dist', binaryName));
    console.log(`✅ Used existing .node file as fallback`);
  }

} catch (error) {
  console.error('❌ Failed to copy native binary:', error.message);
  process.exit(1);
}

// Step 4: Test the built module
console.log('\n🧪 Step 4: Testing built module...');
try {
  // Test import
  const testCode = `
    try {
      const { AudioPlayer, initializeAudio } = require('./dist/index.js');
      console.log('✅ Module imported successfully');

      const result = initializeAudio();
      console.log('✅ Audio system initialized:', result);

      const player = new AudioPlayer();
      console.log('✅ AudioPlayer created');

      console.log('📋 Supported formats:', player.getDevices ? 'OK' : 'Missing');

      console.log('🎉 Build test successful!');
    } catch (error) {
      console.error('❌ Module test failed:', error.message);
      process.exit(1);
    }
  `;

  execSync(`node -e "${testCode.replace(/"/g, '\\"')}"`, { stdio: 'inherit' });

} catch (error) {
  console.error('❌ Module test failed:', error.message);
  console.log('\n🔧 Troubleshooting:');
  console.log('1. Make sure dist/index.js exists');
  console.log('2. Check that native binary is in dist/');
  console.log('3. Verify platform-specific loading logic in index.ts');
  process.exit(1);
}

console.log('\n🎉 Build completed successfully!');
console.log('\n📁 Output files:');
console.log('  - dist/index.js (TypeScript compilation)');
console.log('  - dist/miniaudio-node.*.node (Native binary)');
console.log('  - dist/index.js.map (Source map)');

console.log('\n🔍 To test the examples:');
console.log('  bun examples/javascript/basic.js');
console.log('  bun examples/typescript/advanced.ts');

console.log('\n💡 For development:');
console.log('  bun run dev (hot reload)');
console.log('  bun run test (run tests)');
