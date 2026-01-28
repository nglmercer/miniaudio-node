import {
  AudioPlayer,
  createAudioPlayer,
  isFormatSupported,
} from "../index";
import type { AudioPlayerConfig } from "../index";
import Logger from "./utils/logger";
import { TrackEndReason, TrackEvents, PLAYBACK_DEFAULTS } from "./utils/const";
import type { Song } from "./utils/scanner";

export type Track = string | Buffer;

/**
 * Playlist manager class for handling multiple audio files and buffers
 * with race condition prevention and proper state management.
 */
export class PlaylistManager {
  private player: AudioPlayer;
  private tracks: Track[] = [];
  private songs: Song[] = []; // Store song metadata
  private currentTrackIndex: number = 0;
  private loop: boolean = false;

  // State management for preventing race conditions
  private monitorInterval: NodeJS.Timeout | null = null;
  private isStopping: boolean = false;
  
  // Event callbacks
  public onTrackStart?: (track: Track, index: number) => void;
  public onTrackEnd?: (
    track: Track,
    index: number,
    reason: TrackEndReason,
  ) => void;
  public onPlaylistEnd?: () => void;

  constructor(options?: AudioPlayerConfig | undefined) {
    this.player = createAudioPlayer(options);
  }
  log(name: string, ...args: any[]) {
    Logger.info(name, ...args);
  }
  warn(name: string, ...args: any[]) {
    Logger.warn(name, ...args);
  }
  setIndex(index: number) {
    if (index >= 0 && index < this.tracks.length) {
      this.currentTrackIndex = index;
    }
  }
  /**
   * Load multiple tracks into playlist with metadata scanning
   */
  async loadTracks(tracks: Track[]): Promise<void> {
      const validTracks: Track[] = [];
      for (const track of tracks) {
        if (typeof track === "string") {
          const extension = track.split(".").pop()?.toLowerCase();
          if (extension && isFormatSupported(extension)) {
            validTracks.push(track);
          }
        } else {
          validTracks.push(track);
        }
      }
      this.tracks = validTracks;
    }

  /**
   * Add a single track to the end of the playlist
   */
  async addTrack(track: Track): Promise<void> {
    // Validate track
    if (typeof track === "string") {
      const extension = track.split(".")?.pop()?.toLowerCase();
      if (!extension || !isFormatSupported(extension)) {
        throw new Error(`Unsupported format or invalid extension: ${track}`);
      }

      const fs = await import("node:fs");
      if (!fs.existsSync(track)) {
        throw new Error(`File not found: ${track}`);
      }
    }

    this.tracks.push(track);
    console.log(`✅ Added track. Total: ${this.tracks.length}`);
  }

  /**
   * Load songs with metadata using the library's AudioDecoder
   */
  async loadSongs(songs: Song[]): Promise<void> {
    this.songs = songs;
    this.tracks = songs.map(song => song.path);
    console.log(`✅ Loaded ${songs.length} songs from metadata scan`);
  }

  /**
   * Get metadata for current track using library methods
   */
  getCurrentTrackMetadata(): Song | null {
    if (this.currentTrackIndex >= 0 && this.currentTrackIndex < this.songs.length) {
      return this.songs[this.currentTrackIndex];
    }
    return null;
  }

  /**
   * Get metadata for a specific track index
   */
  getTrackMetadata(index: number): Song | null {
    if (index >= 0 && index < this.songs.length) {
      return this.songs[index];
    }
    return null;
  }

  /**
   * Play current track with lock mechanism to prevent race conditions
   */
 async playCurrentTrack(): Promise<void> {
    if (this.tracks.length === 0) return;

    const track = this.tracks[this.currentTrackIndex];
    try {
      if (typeof track === "string") {
        this.player.loadFile(track);
      } else {
        if (!track){
          this.warn('track is undefined')
          return
        }
        this.player.loadBuffer(Array.from(track));
      }

      // Small delay to ensure miniaudio device is ready
      setTimeout(() => this.player.play(), PLAYBACK_DEFAULTS.SEEK_DELAY_MS);
      this.monitorPlayback();
    } catch (e) {
      console.error("Playback failed", e);
    }
  }

  /**
   * Monitor playback and advance to next track when needed
   */
  private monitorPlayback(): void {
    this.clearMonitorInterval();

    this.monitorInterval = setInterval(() => {
      if (this.isStopping) {
        return;
      }

      try {
        const isPlayerReallyPlaying = this.player.isPlaying();

        if (!isPlayerReallyPlaying) {
          this.clearMonitorInterval();

          const currentTrack = this.tracks[this.currentTrackIndex];
          if (currentTrack && this.onTrackEnd) {
            this.onTrackEnd(currentTrack, this.currentTrackIndex, TrackEndReason.COMPLETED);
          }

          setImmediate(() => {
            if (!this.isStopping) {
              this.nextTrack();
            }
          });
        }
      } catch (error) {
        console.error({error});
        this.clearMonitorInterval();
      }
    }, PLAYBACK_DEFAULTS.MONITOR_INTERVAL_MS);
  }

  /**
   * Clear monitoring interval safely
   */
  private clearMonitorInterval(): void {
    if (this.monitorInterval) {
      clearInterval(this.monitorInterval);
      this.monitorInterval = null;
    }
  }

  /**
   * Play next track in playlist
   */
  async nextTrack(): Promise<void> {

    this.currentTrackIndex++;

    if (this.currentTrackIndex >= this.tracks.length) {
      if (this.loop) {
        this.currentTrackIndex = 0;
        console.log({looping: true});
      } else {
        console.log({end_of_playlist: true});
        this.clearMonitorInterval();

        if (this.onPlaylistEnd) {
          this.onPlaylistEnd();
        }
        return;
      }
    }

    await this.playCurrentTrack();
  }

  /**
   * Play previous track
   */
  async previousTrack(): Promise<void> {

    this.currentTrackIndex = Math.max(0, this.currentTrackIndex - 1);
    await this.playCurrentTrack();
  }

  /**
   * Go to specific track index (0-based)
   */
  async goToTrack(index: number): Promise<void> {
    if (index < 0 || index >= this.tracks.length) {
      throw new Error(`Invalid track index: ${index}`);
    }

    this.currentTrackIndex = index;
    await this.playCurrentTrack();
  }

  /**
   * Pause current playback
   */
  pause(): void {
    try {
      this.player.pause();
      this.clearMonitorInterval();
      console.log("⏸️  Paused");
    } catch (error) {
      console.error("Failed to pause:", error);
    }
  }

  /**
   * Resume playback
   */
  async resume(): Promise<void> {
    if (this.data.duration > 0){

      this.player.play();
      this.monitorPlayback();
      this.log("Resume");
      return;
    }

    if (this.tracks.length > 0) {
      await this.playCurrentTrack();
    }
  }

  /**
   * Stop playback and reset to beginning
   */
  async stop(): Promise<void> {
    this.isStopping = true;
    this.clearMonitorInterval();

    try {
      this.player.stop();
    } catch (error) {
      console.error({error});
    }

    this.currentTrackIndex = 0;
  }

  /**
   * Skip current track immediately
   */
  async skip(): Promise<void> {
    await this.stop();
    await this.nextTrack();
  }

  /**
   * Remove track from playlist
   */
  removeTrack(index: number): Track | null {
    if (index < 0 || index >= this.tracks.length) {
      return null;
    }

    const removed = this.tracks.splice(index, 1)[0];
    this.songs.splice(index, 1);

    if (index < this.currentTrackIndex) {
      this.currentTrackIndex--;
    } else if (index === this.currentTrackIndex) {
      this.stop();
      if (
        this.tracks.length > 0 &&
        this.currentTrackIndex >= this.tracks.length
      ) {
        this.currentTrackIndex = Math.max(0, this.tracks.length - 1);
      }
    }

    console.log(
      {removed: removed, currentTrackIndex: this.currentTrackIndex, tracksLength: this.tracks.length}
    );
    return removed || null;
  }

  /**
   * Set looping mode
   */
  setLoop(enabled: boolean): void {
    this.loop = enabled;
    console.log({loop: this.loop});
  }

  /**
   * Set volume (0.0 to 1.0)
   */
  setVolume(volume: number): void {
    if (volume < 0 || volume > 1) {
      this.warn("Volume must be between 0.0 and 1.0");
      return;
    }
    this.player.setVolume(volume);
    console.log({volume});
  }

  /**
   * Seek to a specific position in the current track (percentage 0.0-1.0)
   */
  async seek(position: number): Promise<void> {
    if (this.tracks.length === 0) {
      this.warn("No tracks to seek in");
      return;
    }

    const duration = this.player.getDuration();
    if (duration <= 0) {
      this.warn("Duration is 0, cannot seek");
      return;
    }

    // position is a percentage (0.0 to 1.0)
    const seekPosition = position;
    if (seekPosition < 0 || seekPosition > 1) {
      this.warn("Seek percentage must be between 0.0 and 1.0", { position });
      return;
    }

    const seekTime = duration * seekPosition;
    try {
      this.player.seekTo(Math.round(seekTime));
      console.log(`✅ Seeked to ${seekTime.toFixed(2)}s / ${duration.toFixed(2)}s (${(seekPosition * 100).toFixed(1)}%)`);
    } catch (error) {
      console.error({error});
    }
  }

  /**
   * Seek to a specific time in seconds
   */
  async seekSeconds(seconds: number): Promise<void> {
    if (this.tracks.length === 0) {
      this.warn("No tracks to seek in");
      return;
    }

    const duration = this.player.getDuration();
    if (duration <= 0) {
      this.warn("Duration is 0, cannot seek");
      return;
    }

    if (seconds < 0 || seconds > duration) {
      this.warn("Seek seconds must be between 0 and duration", { seconds, duration });
      return;
    }

    try {
      this.player.seekTo(Math.round(seconds));
      console.log(`✅ Seeked to ${seconds.toFixed(2)}s / ${duration.toFixed(2)}s`);
    } catch (error) {
      console.error({error});
    }
  }

  /**
   * Get current playback position in seconds
   */
  getCurrentTime(): number {
    return this.player.getCurrentTime();
  }

  /**
   * Get total duration in seconds
   */
  getDuration(): number {
    return this.player.getDuration();
  }

  /**
   * Get playback progress (0.0 to 1.0)
   */
  getProgress(): number {
    const duration = this.getDuration();
    if (duration <= 0) return 0;
    return this.getCurrentTime() / duration;
  }

  /**
   * Save playback state to storage
   */
  async saveState(storage: { set: (key: string, value: any) => Promise<void> }, key: string): Promise<void> {
    const state = {
      currentTrackIndex: this.currentTrackIndex,
      currentTime: this.getCurrentTime(),
      volume: this.getVolume(),
      loop: this.loop,
      timestamp: Date.now(),
    };
    await storage.set(key, state);
    console.log("📁 State saved:", state);
  }

  /**
   * Load playback state from storage
   */
  async loadState(storage: { get: <T>(key: string) => Promise<T | null> }, key: string): Promise<boolean> {
    const state = await storage.get<{
      currentTrackIndex: number;
      currentTime: number;
      volume: number;
      loop: boolean;
      timestamp: number;
    }>(key);
    
    if (!state) {
      console.log("No saved state found");
      return false;
    }
    
    // Restore state
    this.currentTrackIndex = state.currentTrackIndex;
    this.setVolume(state.volume);
    this.loop = state.loop;
    
    console.log("📁 State loaded:", state);
    
    // Seek to saved position if it was recent (within 1 hour)
    const hoursSinceSave = (Date.now() - state.timestamp) / (1000 * 60 * 60);
    if (hoursSinceSave < 1 && this.tracks.length > 0) {
      await this.goToTrack(state.currentTrackIndex);
      if (state.currentTime > 0) {
        // Wait for track to load, then seek
        setTimeout(async () => {
          await this.seekSeconds(state.currentTime);
        }, 200);
      }
      return true;
    }
    
    return false;
  }

  /**
   * Get volume
   */
  getVolume(): number {
    return this.player.getVolume();
  }

  /**
   * Get current track index
   */
  getCurrentIndex(): number {
    return this.currentTrackIndex;
  }

  /**
   * Get total tracks
   */
  getTotalTracks(): number {
    return this.tracks.length;
  }

  /**
   * Get playlist status
   */
  getStatus() {
    const currentTrack = this.tracks[this.currentTrackIndex];
    return {
      totalTracks: this.tracks.length,
      currentTrack: this.currentTrackIndex + 1,
      currentTrackPath:
        typeof currentTrack === "string" ? currentTrack : "Buffer",
      loop: this.loop,
      volume: this.player.getVolume(),
      isStopping: this.isStopping,
    };
  }

  /**
   * Event handlers
   */
  on(
    event: TrackEvents.TRACK_START,
    callback: (track: Track, index: number) => void,
  ): void;
  on(
    event: TrackEvents.TRACK_END,
    callback: (
      track: Track,
      index: number,
      reason: TrackEndReason,
    ) => void,
  ): void;
  on(event: TrackEvents.PLAYLIST_END, callback: () => void): void;
  on(event: string, callback: any): void {
    switch (event) {
      case TrackEvents.TRACK_START:
        this.onTrackStart = callback;
        break;
      case TrackEvents.TRACK_END:
        this.onTrackEnd = callback;
        break;
      case TrackEvents.PLAYLIST_END:
        this.onPlaylistEnd = callback;
        break;
    }
  }

  /**
   * Remove event handlers
   */
  removeListener(event: TrackEvents): void {
    switch (event) {
      case TrackEvents.TRACK_START:
        this.onTrackStart = undefined;
        break;
      case TrackEvents.TRACK_END:
        this.onTrackEnd = undefined;
        break;
      case TrackEvents.PLAYLIST_END:
        this.onPlaylistEnd = undefined;
        break;
    }
  }

  /**
   * Cleanup resources - call this when done using the playlist
   */
  async dispose(): Promise<void> {
    await this.stop();
    this.clearMonitorInterval();
    this.tracks = [];
    this.songs = [];
    this.onTrackStart = undefined;
    this.onTrackEnd = undefined;
    this.onPlaylistEnd = undefined;
    console.log("Playlist disposed");
  }
  get data(){
    const currentTime = this.player.getCurrentTime();
    const duration = this.player.getDuration();
    const volume = this.player.getVolume();
    const isPlaying = this.player.isPlaying();
    const state = this.player.getState();
    const currentTrack = this.player.getCurrentFile();
    return {
      isPlaying,
      currentTime,
      duration,
      progress: currentTime / duration,
      volume,
      state,
      currentTrack
    }
  }
}
