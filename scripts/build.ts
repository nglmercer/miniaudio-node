#!/usr/bin/env bun
/**
 * Build Script for MiniAudio Node - Fixed Version
 *
 * This script orchestrates the complete build process:
 * 1. Clean previous build artifacts
 * 2. Build TypeScript code
 * 3. Build Rust native module
 * 4. Copy native binaries to dist/
 * 5. Generate type declarations
 */

import { execSync } from "node:child_process";
import {
  existsSync,
  rmSync,
  mkdirSync,
  copyFileSync,
  readdirSync,
} from "node:fs";
import { join, dirname, basename } from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const projectRoot = join(__dirname, "..");

interface BuildOptions {
  debug?: boolean;
  verbose?: boolean;
  skipNative?: boolean;
  skipTypes?: boolean;
}

class Builder {
  private options: BuildOptions;
  private startTime: number;

  constructor(options: BuildOptions = {}) {
    this.options = {
      debug: false,
      verbose: false,
      skipNative: false,
      skipTypes: false,
      ...options,
    };
    this.startTime = performance.now();
  }

  /**
   * Log with timestamp
   */
  private log(
    message: string,
    level: "info" | "warn" | "error" = "info",
  ): void {
    const timestamp = new Date().toISOString();
    const prefix = level === "error" ? "❌" : level === "warn" ? "⚠️" : "✅";
    console.log(`[${timestamp}] ${prefix} ${message}`);
  }

  /**
   * Execute command with proper error handling
   */
  private exec(command: string, cwd?: string): string {
    try {
      if (this.options.verbose) {
        this.log(`Executing: ${command}`, "info");
      }

      const result = execSync(command, {
        cwd: cwd || projectRoot,
        stdio: this.options.verbose ? "inherit" : "pipe",
        encoding: "utf-8",
      });

      return result;
    } catch (error) {
      this.log(`Command failed: ${command}`, "error");
      if (error instanceof Error) {
        this.log(`Error: ${error.message}`, "error");
      }
      throw error;
    }
  }

  /**
   * Clean build artifacts
   */
  private clean(): void {
    this.log("Cleaning build artifacts...");

    const dirsToClean = [
      join(projectRoot, "dist"),
      join(projectRoot, "native", "target"),
      join(projectRoot, "coverage"),
      join(projectRoot, ".bun-cache"),
    ];

    dirsToClean.forEach((dir) => {
      if (existsSync(dir)) {
        rmSync(dir, { recursive: true, force: true });
        this.log(`Removed: ${dir}`);
      }
    });

    // Recreate dist directory
    mkdirSync(join(projectRoot, "dist"), { recursive: true });
  }

  /**
   * Build TypeScript code
   */
  private buildTypeScript(): void {
    if (this.options.skipTypes) {
      this.log("Skipping TypeScript build");
      return;
    }

    this.log("Building TypeScript code...");

    // Build with Bun
    const buildCmd = [
      "bun build",
      "src/index.ts",
      "--target node",
      "--outdir dist",
      "--minify",
      this.options.debug ? "--debug" : "",
      "--sourcemap",
    ]
      .filter(Boolean)
      .join(" ");

    this.exec(buildCmd);

    // Generate type declarations
    this.log("Generating type declarations...");
    const typesCmd = [
      "bunx tsc",
      "--project config/tsconfig.json",
      "--emitDeclarationOnly",
      "--declarationMap",
      "--outDir dist",
    ].join(" ");

    this.exec(typesCmd);
  }

  /**
   * Build Rust native module
   */
  private buildNative(): void {
    if (this.options.skipNative) {
      this.log("Skipping native module build");
      return;
    }

    this.log("Building Rust native module...");

    const buildMode = this.options.debug ? "debug" : "release";
    const cargoCmd = [
      `cargo build --${buildMode}`,
      this.options.verbose ? "--verbose" : "",
    ]
      .filter(Boolean)
      .join(" ");

    this.exec(cargoCmd, join(projectRoot, "native"));

    // Copy native binary to dist/
    this.copyNativeBinary(buildMode);
  }

  /**
   * Copy native binary to dist/
   */
  private copyNativeBinary(buildMode: string): void {
    this.log("Copying native binary to dist/");

    const targetDir = join(projectRoot, "native", "target", buildMode);

    // Find the actual native binary file
    let sourceFile: string | null = null;

    // Look for .dll file on Windows
    if (process.platform === "win32") {
      const files = readdirSync(targetDir);
      const dllFile = files.find(
        (f) => f.includes("miniaudio_ffi") && f.endsWith(".dll"),
      );
      if (dllFile) {
        sourceFile = join(targetDir, dllFile);
      }
    } else {
      // Look for .so or .dylib on Unix systems
      const files = readdirSync(targetDir);
      const nativeFile = files.find(
        (f) =>
          f.includes("miniaudio_ffi") &&
          (f.endsWith(".so") || f.endsWith(".dylib")),
      );
      if (nativeFile) {
        sourceFile = join(targetDir, nativeFile);
      }
    }

    if (!sourceFile || !existsSync(sourceFile)) {
      throw new Error(`Native binary not found in ${targetDir}`);
    }

    // Create the correct .node filename
    const destFile = join(projectRoot, "dist", this.getNativeBinaryName());

    copyFileSync(sourceFile, destFile);
    this.log(`Copied: ${sourceFile} -> ${destFile}`);
  }

  /**
   * Get platform-specific native binary name
   */
  private getNativeBinaryName(): string {
    const platform = process.platform;
    const arch = process.arch;

    const platformMap: Record<string, string> = {
      win32: "win32",
      darwin: "darwin",
      linux: "linux",
    };

    const archMap: Record<string, string> = {
      x64: "x64",
      arm64: "arm64",
      ia32: "ia32",
    };

    const normalizedPlatform = platformMap[platform] || platform;
    const normalizedArch = archMap[arch] || arch;

    return `miniaudio-node.${normalizedPlatform}-${normalizedArch}${platform === "win32" ? "-msvc" : ""}.node`;
  }

  /**
   * Generate package.json for distribution
   */
  private generatePackageJson(): void {
    this.log("Generating package.json for distribution...");

    const sourcePackagePath = join(projectRoot, "package.json");
    const distPackagePath = join(projectRoot, "dist", "package.json");

    const packageJson = JSON.parse(this.exec(`cat "${sourcePackagePath}"`));

    // Remove devDependencies and scripts for distribution
    delete packageJson.devDependencies;
    delete packageJson.scripts;

    // Update files field for distribution
    packageJson.files = [
      "index.js",
      "index.d.ts",
      "types/",
      "*.node",
      "README.md",
      "LICENSE",
    ];

    this.exec(
      `echo '${JSON.stringify(packageJson, null, 2)}' > "${distPackagePath}"`,
    );
  }

  /**
   * Copy README and LICENSE to dist/
   */
  private copyMetadata(): void {
    this.log("Copying metadata files...");

    const filesToCopy = [
      { from: "README.md", to: "README.md" },
      { from: "docs/LICENSE", to: "LICENSE" },
    ];

    filesToCopy.forEach(({ from, to }) => {
      const sourcePath = join(projectRoot, from);
      const destPath = join(projectRoot, "dist", to);

      if (existsSync(sourcePath)) {
        copyFileSync(sourcePath, destPath);
        this.log(`Copied: ${from} -> ${to}`);
      } else {
        this.log(`Warning: ${from} not found`, "warn");
      }
    });
  }

  /**
   * Verify build output
   */
  private verifyBuild(): void {
    this.log("Verifying build output...");

    const requiredFiles = [
      "dist/index.js",
      "dist/index.d.ts",
      `dist/${this.getNativeBinaryName()}`,
    ];

    const missingFiles = requiredFiles.filter(
      (file) => !existsSync(join(projectRoot, file)),
    );

    if (missingFiles.length > 0) {
      throw new Error(`Missing required files: ${missingFiles.join(", ")}`);
    }

    this.log("Build verification completed successfully");
  }

  /**
   * Show build summary
   */
  private showSummary(): void {
    const endTime = performance.now();
    const duration = ((endTime - this.startTime) / 1000).toFixed(2);

    this.log("\n🎉 Build Summary:", "info");
    this.log(`⏱️  Duration: ${duration}s`, "info");
    this.log(`🔧 Mode: ${this.options.debug ? "Debug" : "Release"}`, "info");
    this.log(`📁 Output: ${join(projectRoot, "dist")}`, "info");

    // Show file sizes
    const distPath = join(projectRoot, "dist");
    try {
      const files = this.exec(`ls -la "${distPath}"`).trim().split("\n");

      this.log("\n📦 Build artifacts:", "info");
      files.slice(1).forEach((line) => {
        if (line.trim()) {
          this.log(`  ${line}`, "info");
        }
      });
    } catch (error) {
      // If ls fails, just show that build succeeded
      this.log("Build artifacts created successfully", "info");
    }
  }

  /**
   * Run the complete build process
   */
  async build(): Promise<void> {
    try {
      this.log("🚀 Starting MiniAudio Node build process...");

      this.clean();
      this.buildTypeScript();
      this.buildNative();
      this.generatePackageJson();
      this.copyMetadata();
      this.verifyBuild();

      this.showSummary();
    } catch (error) {
      this.log("❌ Build failed!", "error");
      if (error instanceof Error) {
        this.log(`Error: ${error.message}`, "error");
      }
      process.exit(1);
    }
  }
}

// Parse command line arguments
const args = process.argv.slice(2);
const options: BuildOptions = {
  debug: args.includes("--debug") || args.includes("-d"),
  verbose: args.includes("--verbose") || args.includes("-v"),
  skipNative: args.includes("--skip-native"),
  skipTypes: args.includes("--skip-types"),
};

// Show help
if (args.includes("--help") || args.includes("-h")) {
  console.log(`
MiniAudio Node Build Script

Usage:
  bun scripts/build.ts [options]

Options:
  --debug, -d          Build in debug mode
  --verbose, -v        Show verbose output
  --skip-native        Skip native module build
  --skip-types         Skip TypeScript build
  --help, -h           Show this help message

Examples:
  bun scripts/build.ts                    # Release build
  bun scripts/build.ts --debug            # Debug build
  bun scripts/build.ts --verbose          # Verbose output
  bun scripts/build.ts --skip-types       # Skip TypeScript
  bun scripts/build.ts --skip-native       # Skip native module
`);
  process.exit(0);
}

// Run build
const builder = new Builder(options);
await builder.build();
