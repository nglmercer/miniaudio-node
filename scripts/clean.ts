#!/usr/bin/env bun
/**
 * Clean Script for MiniAudio Node
 *
 * This script removes all build artifacts and temporary files:
 * - dist/ directory
 * - native/target/ directory
 * - coverage/ directory
 * - .bun-cache/ directory
 * - node_modules/ (optional)
 */

import { existsSync, rmSync, statSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)
const projectRoot = join(__dirname, '..')

interface CleanOptions {
  verbose?: boolean
  dryRun?: boolean
  aggressive?: boolean // Remove node_modules as well
}

class Cleaner {
  private options: CleanOptions
  private cleaned: string[] = []
  private errors: string[] = []

  constructor(options: CleanOptions = {}) {
    this.options = {
      verbose: false,
      dryRun: false,
      aggressive: false,
      ...options
    }
  }

  /**
   * Log with appropriate emoji
   */
  private log(message: string, type: 'info' | 'success' | 'warn' | 'error' = 'info'): void {
    const icons = {
      info: '📋',
      success: '✅',
      warn: '⚠️',
      error: '❌'
    }
    console.log(`${icons[type]} ${message}`)
  }

  /**
   * Get directory size in human readable format
   */
  private getDirectorySize(dirPath: string): string {
    if (!existsSync(dirPath)) return '0 B'

    try {
      const stat = statSync(dirPath)
      if (!stat.isDirectory()) return this.formatBytes(stat.size)

      let totalSize = 0
      const items = this.getAllFiles(dirPath)

      for (const item of items) {
        try {
          totalSize += statSync(item).size
        } catch {
          // Skip files we can't stat
        }
      }

      return this.formatBytes(totalSize)
    } catch {
      return 'Unknown'
    }
  }

  /**
   * Get all files recursively in directory
   */
  private getAllFiles(dirPath: string, arrayOfFiles: string[] = []): string[] {
    const fs = require('fs')
    const files = fs.readdirSync(dirPath)

    files.forEach((file: string) => {
      const fullPath = join(dirPath, file)
      if (fs.statSync(fullPath).isDirectory()) {
        arrayOfFiles = this.getAllFiles(fullPath, arrayOfFiles)
      } else {
        arrayOfFiles.push(fullPath)
      }
    })

    return arrayOfFiles
  }

  /**
   * Format bytes to human readable format
   */
  private formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B'

    const k = 1024
    const sizes = ['B', 'KB', 'MB', 'GB']
    const i = Math.floor(Math.log(bytes) / Math.log(k))

    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i]
  }

  /**
   * Remove directory or file safely
   */
  private remove(path: string): boolean {
    if (!existsSync(path)) {
      this.log(`Skipping (not found): ${path}`, 'warn')
      return false
    }

    const size = this.getDirectorySize(path)

    if (this.options.verbose || this.options.dryRun) {
      this.log(`Found: ${path} (${size})`, 'info')
    }

    if (this.options.dryRun) {
      this.log(`[DRY RUN] Would remove: ${path} (${size})`, 'warn')
      this.cleaned.push(path)
      return true
    }

    try {
      rmSync(path, { recursive: true, force: true })
      this.log(`Removed: ${path} (${size})`, 'success')
      this.cleaned.push(path)
      return true
    } catch (error) {
      const errorMsg = `Failed to remove ${path}: ${error instanceof Error ? error.message : 'Unknown error'}`
      this.log(errorMsg, 'error')
      this.errors.push(errorMsg)
      return false
    }
  }

  /**
   * Clean build artifacts
   */
  private cleanBuildArtifacts(): void {
    this.log('\n🧹 Cleaning build artifacts...', 'info')

    const artifacts = [
      'dist',
      'native/target',
      'coverage',
      '.bun-cache',
      'test-results',
      'playwright-report'
    ]

    artifacts.forEach(artifact => {
      const path = join(projectRoot, artifact)
      this.remove(path)
    })
  }

  /**
   * Clean temporary files
   */
  private cleanTempFiles(): void {
    this.log('\n🗑️ Cleaning temporary files...', 'info')

    const tempFiles = [
      '*.log',
      '*.tmp',
      '*.temp',
      '.eslintcache',
      '*.tsbuildinfo',
      '.DS_Store',
      'Thumbs.db'
    ]

    // Note: This would require glob matching, simplified for now
    const tempPaths = [
      join(projectRoot, '.eslintcache'),
      join(projectRoot, 'bun.lockb') // Only if aggressive cleaning
    ]

    tempPaths.forEach(path => {
      if (this.options.aggressive || !path.includes('bun.lockb')) {
        this.remove(path)
      }
    })
  }

  /**
   * Clean development dependencies
   */
  private cleanDependencies(): void {
    if (!this.options.aggressive) {
      this.log('\n⚠️ Skipping node_modules (use --aggressive to include)', 'warn')
      return
    }

    this.log('\n📦 Cleaning development dependencies...', 'info')

    const nodeModulesPath = join(projectRoot, 'node_modules')
    this.remove(nodeModulesPath)
  }

  /**
   * Clean old lock files
   */
  private cleanLockFiles(): void {
    this.log('\n🔒 Cleaning lock files...', 'info')

    const lockFiles = [
      'package-lock.json',
      'yarn.lock',
      'pnpm-lock.yaml'
    ]

    lockFiles.forEach(file => {
      const path = join(projectRoot, file)
      this.remove(path)
    })
  }

  /**
   * Show cleaning summary
   */
  private showSummary(): void {
    this.log('\n📊 Cleaning Summary:', 'info')

    if (this.cleaned.length > 0) {
      this.log(`✅ Cleaned ${this.cleaned.length} items:`, 'success')
      this.cleaned.forEach(item => {
        if (this.options.verbose) {
          this.log(`  - ${item}`, 'info')
        }
      })
    } else {
      this.log('📭 No items were cleaned', 'info')
    }

    if (this.errors.length > 0) {
      this.log(`\n❌ ${this.errors.length} errors:`, 'error')
      this.errors.forEach(error => {
        this.log(`  - ${error}`, 'error')
      })
    }

    // Show disk space freed (approximation)
    if (!this.options.dryRun && this.cleaned.length > 0) {
      this.log('\n💡 Tip: Run "bun install" to restore dependencies', 'info')
    }
  }

  /**
   * Run complete cleaning process
   */
  async clean(): Promise<void> {
    try {
      this.log('🚀 Starting MiniAudio Node cleanup...', 'info')

      if (this.options.dryRun) {
        this.log('🔍 DRY RUN MODE - No files will be actually removed', 'warn')
      }

      this.cleanBuildArtifacts()
      this.cleanTempFiles()
      this.cleanLockFiles()
      this.cleanDependencies()
      this.showSummary()

      if (this.errors.length > 0) {
        process.exit(1)
      }

    } catch (error) {
      this.log('❌ Cleanup failed!', 'error')
      if (error instanceof Error) {
        this.log(`Error: ${error.message}`, 'error')
      }
      process.exit(1)
    }
  }
}

// Parse command line arguments
const args = process.argv.slice(2)
const options: CleanOptions = {
  verbose: args.includes('--verbose') || args.includes('-v'),
  dryRun: args.includes('--dry-run') || args.includes('-n'),
  aggressive: args.includes('--aggressive') || args.includes('-a')
}

// Show help
if (args.includes('--help') || args.includes('-h')) {
  console.log(`
MiniAudio Node Clean Script

Usage:
  bun scripts/clean.ts [options]

Options:
  --verbose, -v        Show verbose output
  --dry-run, -n        Show what would be removed without actually removing
  --aggressive, -a     Remove node_modules and other development files
  --help, -h          Show this help message

Examples:
  bun scripts/clean.ts                    # Standard cleanup
  bun scripts/clean.ts --verbose          # Show details
  bun scripts/clean.ts --dry-run          # Preview what would be cleaned
  bun scripts/clean.ts --aggressive       # Remove everything including node_modules
  bun scripts/clean.ts -van               # Verbose aggressive dry run
`)
  process.exit(0)
}

// Run cleanup
const cleaner = new Cleaner(options)
await cleaner.clean()
