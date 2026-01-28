export enum TrackEndReason {
    COMPLETED = "completed",
    MANUAL = "manual",
}
export enum TrackEvents {
    TRACK_START = "trackStart",
    TRACK_END = "trackEnd",
    PLAYLIST_END = "playlistEnd",
}

// Storage keys
export const STORAGE_KEYS = {
    PLAYER_STATE: "player_state",
    SONGS: "songs",
} as const;

// UI paths
export const UI_PATHS = {
    MAIN_WINDOW: "src/ui/main.slint",
} as const;

// Playback defaults
export const PLAYBACK_DEFAULTS = {
    DEFAULT_VOLUME: 0.7,
    PROGRESS_INTERVAL_MS: 500,
    MONITOR_INTERVAL_MS: 500,
    SEEK_DELAY_MS: 100,
} as const;
