import { PlaylistManager } from './playlist';
import { storage } from './utils/storage';
import { scanExternalFolder } from './utils/scanner';
import { STORAGE_KEYS, PLAYBACK_DEFAULTS, TrackEvents, TrackEndReason } from './utils/const';
import type { Song } from './utils/scanner';
import type { Track } from './playlist';

const playlist = new PlaylistManager({ volume: PLAYBACK_DEFAULTS.DEFAULT_VOLUME });
const musicPath = "/home/meme/Música/";

// Track current state for auto-save
let stateSaveInterval: NodeJS.Timeout | null = null;
let isPlaying = false;

/**
 * Initialize the music player
 */
async function init(): Promise<void> {
    console.log("🎵 Initializing music player...");
    
    // Set up event handlers
    setupEventHandlers();
    
    // Load saved songs or scan directory
    await loadOrScanMusic();
    
    // Try to restore playback state
    const restored = await playlist.loadState(storage, STORAGE_KEYS.PLAYER_STATE);
    if (restored) {
        console.log("✅ Playback state restored!");
    }
    
    // Start auto-save state every 10 seconds
    startAutoSave();
    
    // Print initial status
    printStatus();
    
    console.log("🎵 Music player ready!");
}

/**
 * Load existing songs or scan directory
 */
async function loadOrScanMusic(): Promise<void> {
    // Check if we have saved songs
    const savedSongs = await storage.get<Song[]>(STORAGE_KEYS.SONGS);
    
    if (savedSongs && savedSongs.length > 0) {
        console.log(`📚 Found ${savedSongs.length} saved songs`);
        playlist.loadSongs(savedSongs);
        
        // Optionally re-scan to update metadata
        // Uncomment the following lines to re-scan on each start:
        // const newTracks = await scanExternalFolder(musicPath);
        // if (newTracks.length > 0) {
        //     console.log(`🔄 Updated with ${newTracks.length} tracks`);
        //     playlist.loadSongs(newTracks);
        //     await storage.set(STORAGE_KEYS.SONGS, newTracks);
        // }
    } else {
        console.log("🔍 Scanning music folder...");
        const tracks = await scanExternalFolder(musicPath);
        if (tracks.length > 0) {
            console.log(`✅ Found ${tracks.length} tracks`);
            playlist.loadSongs(tracks);
            await storage.set(STORAGE_KEYS.SONGS, tracks);
        } else {
            console.log("⚠️ No tracks found. Make sure the music path exists.");
        }
    }
}

/**
 * Set up event handlers
 */
function setupEventHandlers(): void {
    playlist.on(TrackEvents.TRACK_START, (track: Track, index: number) => {
        const metadata = playlist.getTrackMetadata(index);
        const trackName = typeof track === "string" ? track.split("/").pop() : "Buffer";
        console.log(`▶️  Playing: ${metadata?.title || trackName} (${index + 1}/${playlist.getTotalTracks()})`);
        isPlaying = true;
    });
    
    playlist.on(TrackEvents.TRACK_END, (track: Track, index: number, reason: TrackEndReason) => {
        console.log(`⏹️  Track ended: ${index}, reason: ${reason}`,track);
        isPlaying = false;
    });
    
    playlist.on(TrackEvents.PLAYLIST_END, () => {
        console.log("📋 Playlist ended");
        isPlaying = false;
    });
}

/**
 * Start auto-save interval
 */
function startAutoSave(): void {
    stateSaveInterval = setInterval(async () => {
        if (playlist.getTotalTracks() > 0) {
            await playlist.saveState(storage, STORAGE_KEYS.PLAYER_STATE);
        }
    }, 10000); // Save every 10 seconds
}

/**
 * Print current status
 */
function printStatus(): void {
    const status = playlist.getStatus();
    const metadata = playlist.getCurrentTrackMetadata();
    
    console.log("\n=== Player Status ===");
    console.log(`Tracks: ${status.totalTracks}`);
    console.log(`Current: ${status.currentTrack} - ${metadata?.title || "Unknown"}`);
    console.log(`Artist: ${metadata?.artist || "Unknown"}`);
    console.log(`Duration: ${formatTime(metadata?.duration || 0)}`);
    console.log(`Volume: ${Math.round(status.volume * 100)}%`);
    console.log(`Loop: ${status.loop ? "On" : "Off"}`);
    console.log("====================\n");
}

/**
 * Format seconds to MM:SS
 */
function formatTime(seconds: number): string {
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${mins}:${secs.toString().padStart(2, "0")}`;
}

/**
 * Print help for available commands
 */
function printHelp(): void {
    console.log(`
=== Available Commands ===
play                  - Play/Resume
pause                 - Pause
stop                  - Stop and reset
next                  - Next track
prev                  - Previous track
goto <n>              - Go to track <n> (1-based)
seek <seconds>        - Seek to position in seconds (e.g., seek 50)
seekp <0-1>           - Seek to position in percentage (e.g., seekp 0.5)
loop <on|off>         - Toggle loop mode
volume <0-100>        - Set volume
status                - Print current status
info                  - Print track info
save                  - Force save state
quit                  - Exit
=========================
`);
}

/**
 * Main entry point with interactive commands
 */
async function main(): Promise<void> {
    try {
        await init();
        printHelp();
        
        // Start playing first track if not restored
        if (playlist.getTotalTracks() > 0 && !isPlaying) {
            console.log("▶️  Starting playback...");
            await playlist.playCurrentTrack();
        }
        
        // Interactive mode using readline
        const readline = await import("readline");
        const rl = readline.createInterface({
            input: process.stdin,
            output: process.stdout,
            terminal: true
        });
        
        console.log("Type 'help' for available commands, 'quit' to exit.\n");
        
        rl.on("line", async (line: string) => {
            const command = line.trim().toLowerCase();
            
            switch (command) {
                case "help":
                    printHelp();
                    break;
                case "play":
                    await playlist.resume();
                    break;
                case "pause":
                    playlist.pause();
                    break;
                case "stop":
                    await playlist.stop();
                    await playlist.saveState(storage, STORAGE_KEYS.PLAYER_STATE);
                    break;
                case "next":
                    await playlist.nextTrack();
                    break;
                case "prev":
                    await playlist.previousTrack();
                    break;
                case "quit":
                case "exit":
                    console.log("👋 Saving state and exiting...");
                    await playlist.saveState(storage, STORAGE_KEYS.PLAYER_STATE);
                    if (stateSaveInterval) clearInterval(stateSaveInterval);
                    await playlist.dispose();
                    rl.close();
                    process.exit(0);
                case "save":
                    await playlist.saveState(storage, STORAGE_KEYS.PLAYER_STATE);
                    console.log("💾 State saved!");
                    break;
                case "status":
                    printStatus();
                    break;
                case "info":
                    const metadata = playlist.getCurrentTrackMetadata();
                    if (metadata) {
                        console.log(`
=== Current Track ===
Title: ${metadata.title}
Artist: ${metadata.artist}
Album: ${metadata.album}
Duration: ${formatTime(metadata.duration)}
Sample Rate: ${metadata.sampleRate} Hz
Channels: ${metadata.channels}
Path: ${metadata.path}
=====================
`);
                    }
                    break;
                default:
                    if (command.startsWith("goto ")) {
                        const parts = command.split(" ");
                        const index = parseInt(parts[1]) - 1;
                        await playlist.goToTrack(index);
                    } else if (command.startsWith("seek ")) {
                        const parts = command.split(" ");
                        const seconds = parseFloat(parts[1]);
                        await playlist.seekSeconds(seconds);
                    } else if (command.startsWith("seekp ")) {
                        const parts = command.split(" ");
                        const percent = parseFloat(parts[1]);
                        await playlist.seek(percent);
                    } else if (command.startsWith("volume ")) {
                        const parts = command.split(" ");
                        const volume = parseInt(parts[1]) / 100;
                        playlist.setVolume(volume);
                    } else if (command.startsWith("loop ")) {
                        const parts = command.split(" ");
                        const enabled = parts[1] === "on";
                        playlist.setLoop(enabled);
                    } else if (command) {
                        console.log(`Unknown command: ${command}. Type 'help' for available commands.`);
                    }
                    break;
            }
        });
    } catch (error) {
        console.error({error});
    }
}
main()
// Handle graceful shutdown
process.on("SIGINT", async () => {
    console.log("\n👋 Saving state and exiting...");
    await playlist.saveState(storage, STORAGE_KEYS.PLAYER_STATE);
    if (stateSaveInterval) clearInterval(stateSaveInterval);
    await playlist.dispose();
    process.exit(0);
});
