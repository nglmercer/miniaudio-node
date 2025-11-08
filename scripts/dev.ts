#!/usr/bin/env bun
/**
 * Development Script for MiniAudio Node
 *
 * This script provides a development workflow with:
 * - Watch mode for TypeScript compilation
 * - Hot reloading of native module
 * - Development server setup
 * - Live testing integration
 */

import { execSync, spawn } from 'node:child_process'
import { existsSync, watchFile, unwatchFile } from 'node:fs'
import { join, dirname, relative } from 'node:path'
import { fileURLToPath } from 'node:url'
import { debouncer } from 'bun-debounce'

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)
const projectRoot = join(__dirname, '..')

interface DevOptions {
  port?: number
  verbose?: boolean
  skipTests?: boolean
  skipNative?: boolean
  rebuildOnChange?: boolean
}

class DevServer {
  private options: DevOptions
  private watchers: Array<() => void> = []
  private processes: Array<any> = []
  private isBuilding = false

  constructor(options: DevOptions = {}) {
    this.options = {
      port: 3000,
      verbose: false,
      skipTests: false,
      skipNative: false,
      rebuildOnChange: true,
      ...options
    }
  }

  /**
   * Log with timestamp and emoji
   */
  private log(message: string, type: 'info' | 'success' | 'warn' | 'error' = 'info'): void {
    const timestamp = new Date().toLocaleTimeString()
    const icons = {
      info: '📋',
      success: '✅',
      warn: '⚠️',
      error: '❌'
    }
    console.log(`[${timestamp}] ${icons[type]} ${message}`)
  }

  /**
   * Execute command with proper error handling
   */
  private exec(command: string, cwd?: string): Promise<string> {
    return new Promise((resolve, reject) => {
      const [cmd, ...args] = command.split(' ')

      const child = spawn(cmd, args, {
        cwd: cwd || projectRoot,
        stdio: this.options.verbose ? 'inherit' : 'pipe',
        shell: true
      })

      let output = ''
      let error = ''

      child.stdout?.on('data', (data) => {
        output += data.toString()
      })

      child.stderr?.on('data', (data) => {
        error += data.toString()
      })

      child.on('close', (code) => {
        if (code === 0) {
          resolve(output)
        } else {
          reject(new Error(`Command failed: ${command}\n${error}`))
        }
      })

      child.on('error', reject)
    })
  }

  /**
   * Build TypeScript in watch mode
   */
  private async startTypeScriptWatcher(): Promise<void> {
    this.log('Starting TypeScript watcher...')

    const watchCmd = [
      'bun --watch',
      'src/index.ts',
      '--target node',
      '--outdir dist',
      '--sourcemap'
    ].join(' ')

    const tsProcess = spawn('bun', [
      '--watch',
      'src/index.ts',
      '--target', 'node',
      '--outdir', 'dist',
      '--sourcemap'
    ], {
      cwd: projectRoot,
      stdio: ['inherit', 'pipe', 'pipe']
    })

    tsProcess.stdout?.on('data', (data) => {
      const output = data.toString()
      if (this.options.verbose) {
        console.log(output)
      }

      if (output.includes('Build success')) {
        this.log('TypeScript compiled successfully', 'success')
        this.runTestsOnChange()
      }
    })

    tsProcess.stderr?.on('data', (data) => {
      console.error(data.toString())
    })

    this.processes.push(tsProcess)
    this.log('TypeScript watcher started', 'success')
  }

  /**
   * Watch Rust source files for changes
   */
  private startNativeWatcher(): void {
    if (this.options.skipNative) {
      this.log('Skipping native module watcher', 'warn')
      return
    }

    this.log('Starting native module watcher...')

    const rustSrcDir = join(projectRoot, 'native', 'src')
    const rustFiles = ['lib.rs'].map(file => join(rustSrcDir, file))

    let rebuildTimeout: NodeJS.Timeout | null = null

    const rebuildNative = debouncer(async () => {
      if (this.isBuilding) return

      this.isBuilding = true
      this.log('🔧 Rebuilding native module...')

      try {
        await this.exec('cargo build --release', join(projectRoot, 'native'))
        await this.copyNativeBinary()
        this.log('Native module rebuilt successfully', 'success')
        this.runTestsOnChange()
      } catch (error) {
        this.log(`Native rebuild failed: ${error}`, 'error')
      } finally {
        this.isBuilding = false
      }
    }, 1000) // Wait 1 second after file changes

    rustFiles.forEach(file => {
      if (existsSync(file)) {
        watchFile(file, { interval: 500 }, () => {
          this.log(`📝 Native source changed: ${relative(projectRoot, file)}`)
          rebuildNative()
        })

        this.watchers.push(() => unwatchFile(file))
      }
    })

    this.log('Native module watcher started', 'success')
  }

  /**
   * Copy native binary after build
   */
  private async copyNativeBinary(): Promise<void> {
    const nativeTarget = join(projectRoot, 'native', 'target', 'release')
    const binaryName = this.getNativeBinaryName()
    const sourcePath = join(nativeTarget, `${binaryName}.node`)
    const destPath = join(projectRoot, 'dist', `${binaryName}.node`)

    if (!existsSync(sourcePath)) {
      throw new Error(`Native binary not found: ${sourcePath}`)
    }

    await this.exec(`cp "${sourcePath}" "${destPath}"`)
  }

  /**
   * Get platform-specific native binary name
   */
  private getNativeBinaryName(): string {
    const platform = process.platform
    const arch = process.arch

    const platformMap: Record<string, string> = {
      'win32': 'win32',
      'darwin': 'darwin',
      'linux': 'linux'
    }

    const archMap: Record<string, string> = {
      'x64': 'x64',
      'arm64': 'arm64',
      'ia32': 'ia32'
    }

    const normalizedPlatform = platformMap[platform] || platform
    const normalizedArch = archMap[arch] || arch

    return `miniaudio-node.${normalizedPlatform}-${normalizedArch}${platform === 'win32' ? '-msvc' : ''}`
  }

  /**
   * Run tests on file changes
   */
  private runTestsOnChange(): void {
    if (this.options.skipTests) return

    this.log('🧪 Running tests...')

    // Run tests in background
    this.exec('bun test --quiet').catch(error => {
      this.log(`Tests failed: ${error}`, 'warn')
    })
  }

  /**
   * Start development server
   */
  private startDevServer(): void {
    this.log(`Starting development server on port ${this.options.port}...`)

    // Simple file server for examples
    const server = Bun.serve({
      port: this.options.port,
      root: join(projectRoot, 'examples'),
      fetch(req) {
        const url = new URL(req.url)

        // Serve examples directory
        if (url.pathname === '/') {
          return new Response(`
<!DOCTYPE html>
<html>
<head>
  <title>MiniAudio Node Development</title>
  <style>
    body { font-family: system-ui; margin: 2rem; }
    .example { border: 1px solid #ccc; padding: 1rem; margin: 1rem 0; border-radius: 8px; }
    .example h3 { margin-top: 0; color: #333; }
    .example p { color: #666; }
    a { color: #007acc; text-decoration: none; }
    a:hover { text-decoration: underline; }
  </style>
</head>
<body>
  <h1>🎵 MiniAudio Node Development Server</h1>
  <p>Examples are available below. Check the console for detailed output.</p>

  <div class="example">
    <h3>JavaScript Examples</h3>
    <p>Basic usage examples in JavaScript</p>
    <a href="/javascript/basic.js">Basic Example</a>
  </div>

  <div class="example">
    <h3>TypeScript Examples</h3>
    <p>Advanced examples with TypeScript and type safety</p>
    <a href="/typescript/advanced.ts">Advanced Example</a>
  </div>

  <div class="example">
    <h3>API Documentation</h3>
    <p>API reference and documentation</p>
    <a href="/docs/api">API Docs</a>
  </div>

  <script>
    console.log('🚀 MiniAudio Node Development Server Ready')
    console.log('📝 Edit src/ files to see live changes')
    console.log('🔧 Edit native/src/ files to rebuild the native module')
  </script>
</body>
</html>
          `, {
            headers: { 'Content-Type': 'text/html' }
          })
        }

        // Try to serve static files
        const filePath = join(projectRoot, 'examples', url.pathname)
        try {
          const file = Bun.file(filePath)
          return new Response(file)
        } catch {
          return new Response('File not found', { status: 404 })
        }
      }
    })

    this.processes.push({ kill: () => server.stop() })
    this.log(`Development server running at http://localhost:${this.options.port}`, 'success')
  }

  /**
   * Show development status
   */
  private showStatus(): void {
    console.log('\n🎛️ Development Server Status:')
    console.log('===============================')
    console.log(`📁 Project: ${projectRoot}`)
    console.log(`🌐 Server: http://localhost:${this.options.port}`)
    console.log(`📝 TypeScript: Watching`)
    console.log(`🦀 Native Module: ${this.options.skipNative ? 'Disabled' : 'Watching'}`)
    console.log(`🧪 Auto Tests: ${this.options.skipTests ? 'Disabled' : 'Enabled'}`)
    console.log(`📊 Verbose: ${this.options.verbose ? 'Enabled' : 'Disabled'}`)
    console.log('\n💡 Tips:')
    console.log('  - Edit src/*.ts files to see live compilation')
    console.log('  - Edit native/src/*.rs files to rebuild native module')
    console.log('  - Press Ctrl+C to stop the development server')
    console.log('  - Check browser console for example output')
    console.log('  - Tests run automatically on file changes')
    console.log('===============================\n')
  }

  /**
   * Handle cleanup on exit
   */
  private setupCleanup(): void {
    const cleanup = () => {
      this.log('\n🛑 Shutting down development server...')

      // Stop all processes
      this.processes.forEach(process => {
        try {
          if (process.kill) process.kill()
        } catch (error) {
          // Ignore cleanup errors
        }
      })

      // Stop all watchers
      this.watchers.forEach(unwatch => {
        try {
          unwatch()
        } catch (error) {
          // Ignore cleanup errors
        }
      })

      this.log('Development server stopped', 'success')
      process.exit(0)
    }

    process.on('SIGINT', cleanup)
    process.on('SIGTERM', cleanup)
    process.on('SIGQUIT', cleanup)
  }

  /**
   * Start development environment
   */
  async start(): Promise<void> {
    try {
      this.log('🚀 Starting MiniAudio Node development environment...')

      // Ensure dist directory exists
      await this.exec('mkdir -p dist')

      // Initial build
      this.log('🔧 Building project...')
      await this.exec('bun scripts/build.ts --debug')

      // Start all development services
      await this.startTypeScriptWatcher()
      this.startNativeWatcher()
      this.startDevServer()

      this.setupCleanup()
      this.showStatus()

      this.log('✅ Development environment ready!', 'success')

    } catch (error) {
      this.log(`❌ Failed to start development server: ${error}`, 'error')
      process.exit(1)
    }
  }
}

// Parse command line arguments
const args = process.argv.slice(2)
const options: DevOptions = {
  port: args.find(arg => arg.startsWith('--port='))?.split('=')[1]
    ? parseInt(args.find(arg => arg.startsWith('--port='))!.split('=')[1])
    : undefined,
  verbose: args.includes('--verbose') || args.includes('-v'),
  skipTests: args.includes('--skip-tests'),
  skipNative: args.includes('--skip-native'),
  rebuildOnChange: !args.includes('--no-rebuild')
}

// Show help
if (args.includes('--help') || args.includes('-h')) {
  console.log(`
MiniAudio Node Development Server

Usage:
  bun scripts/dev.ts [options]

Options:
  --port=<number>      Port for development server (default: 3000)
  --verbose, -v         Show verbose output
  --skip-tests         Don't run tests automatically
  --skip-native         Don't watch native module files
  --no-rebuild          Don't rebuild on native file changes
  --help, -h           Show this help message

Examples:
  bun scripts/dev.ts                    # Default development server
  bun scripts/dev.ts --port=8080       # Custom port
  bun scripts/dev.ts --verbose         # Verbose output
  bun scripts/dev.ts --skip-tests      # Skip auto-tests
`)
  process.exit(0)
}

// Start development server
const devServer = new DevServer(options)
await devServer.start()
