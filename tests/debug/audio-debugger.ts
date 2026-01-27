import * as fs from 'fs';
import * as path from 'path';
import { AudioPlayer, initializeAudio, getSupportedFormats, setDebug } from '../../index.js';

setDebug(true);

class AudioDebugger {
  private player = new AudioPlayer();
  private filePath: string;

  constructor(filePath: string) {
    this.filePath = path.resolve(filePath);
  }

  private async checkMagicBytes() {
    const buffer = Buffer.alloc(12);
    const fd = fs.openSync(this.filePath, 'r');
    fs.readSync(fd, buffer, 0, 12, 0);
    fs.closeSync(fd);
    return {
      hex: buffer.toString('hex'),
      ascii: buffer.toString('ascii').replace(/[^\x20-\x7E]/g, '.'),
    };
  }

  async run() {
    const stats = fs.existsSync(this.filePath) ? fs.statSync(this.filePath) : null;
    
    const report = {
      timestamp: new Date().toISOString(),
      env: { init: initializeAudio(), formats: getSupportedFormats() },
      file: {
        path: this.filePath,
        exists: !!stats,
        size: stats?.size || 0,
        extension: path.extname(this.filePath).slice(1),
        header: stats ? await this.checkMagicBytes() : null
      },
      playback: {
        loaded: false,
        playing: false,
        state: 'IDLE',
        error: null as any
      }
    };

    if (!stats) return { ...report, error: "FILE_NOT_FOUND" };

    try {
      this.player.loadFile(this.filePath);
      report.playback.loaded = true;
      
      this.player.play();
      await new Promise(r => setTimeout(r, 1500)); // Espera validación técnica

      report.playback.playing = this.player.isPlaying();
      report.playback.state = this.player.getState();
      
      this.player.stop();
    } catch (e: any) {
      report.playback.error = { message: e.message, stack: e.stack };
    }

    return report;
  }
}

// Ejecución directa: Solo imprime el objeto final
if (import.meta.main) {
  const target = process.argv[2];
  if (!target) {
    console.error({ error: "MISSING_PATH", usage: "bun run debug.ts <path>" });
    process.exit(1);
  }

  new AudioDebugger(target).run()
    .then(result => {
      console.dir(result, { depth: null, colors: true });
      process.exit(result.playback.playing ? 0 : 1);
    })
    .catch(err => {
      console.error({ fatal: err.message });
      process.exit(1);
    });
}