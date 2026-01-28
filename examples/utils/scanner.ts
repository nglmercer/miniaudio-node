import { Glob } from "bun";
import { AudioDecoder, getAudioMetadata, isFormatSupported } from "../../index";
import { join, isAbsolute, resolve } from "node:path";

export interface Song {
    title: string;
    artist: string;
    album: string;
    duration: number;
    'cover-url': string;
    path: string; // file path to audio
    sampleRate: number;
    channels: number;
}

export async function scanExternalFolder(externalPath: string): Promise<Song[]> {
    // 1. Normalize the path. 
    // If it starts with "home/...", resolve("/") prepends the root slash.
    const absolutePath = isAbsolute(externalPath) 
        ? externalPath 
        : resolve("/", externalPath);

    // 2. Instantiate Glob with the audio extensions
    const glob = new Glob("**/*.{mp3,flac,m4a,wav,ogg,opus,aac}");
    const songs: Song[] = [];

    try {
        // 3. Use 'cwd' in scan options to point to the absolute music folder
        // We set 'onlyFiles: true' to ignore directories
        const scanner = glob.scan({ 
            cwd: absolutePath, 
            onlyFiles: true,
            absolute: false // We keep it false so 'file' is the relative path from the music folder
        });

        for await (const file of scanner) {
            const fullPath = join(absolutePath, file);
            
            try {
                // Use the library's AudioDecoder to get accurate audio properties
                const decoder = new AudioDecoder(fullPath);
                
                // Get duration from the decoder
                const duration = decoder.getDuration();
                
                // Get audio metadata from library (placeholder, but provides structure)
                const metadata = getAudioMetadata(fullPath);
                
                // Extract filename-based metadata as fallback
                const fileName = file.replace(/\.[^/.]+$/, "");
                const title = metadata.title || formatTitle(fileName);
                const artist = metadata.artist || extractArtist(fileName);
                const album = metadata.album || extractAlbum(fileName);

                songs.push({
                    title,
                    artist,
                    album,
                    duration: Math.round(duration),
                    'cover-url': "", // Library doesn't support cover art extraction
                    path: fullPath,
                    sampleRate: decoder.getSampleRate(),
                    channels: decoder.getChannels(),
                });
            } catch (err) {
                // Skip files that aren't valid audio or are corrupted
                console.error(`Skipping (invalid audio): ${fullPath}`, err);
            }
        }
    } catch (err) {
        console.error(`Failed to access directory: ${absolutePath}`, err);
    }

    return songs;
}

/**
 * Scan a single audio file and return its metadata using library methods
 */
export async function scanSingleFile(filePath: string): Promise<Song | null> {
    try {
        const decoder = new AudioDecoder(filePath);
        const metadata = getAudioMetadata(filePath);
        
        const fileName = filePath.split("/").pop()?.replace(/\.[^/.]+$/, "") || "";
        
        return {
            title: metadata.title || formatTitle(fileName),
            artist: metadata.artist || "Unknown",
            album: metadata.album || "Unknown",
            duration: Math.round(decoder.getDuration()),
            'cover-url': "",
            path: filePath,
            sampleRate: decoder.getSampleRate(),
            channels: decoder.getChannels(),
        };
    } catch (err) {
        console.error(`Failed to scan file: ${filePath}`, err);
        return null;
    }
}

/**
 * Format file name into a title (remove underscores, hyphens, etc.)
 */
function formatTitle(fileName: string): string {
    return fileName
        .replace(/[-_]/g, " ")
        .replace(/\s+/g, " ")
        .trim()
        .split(" ")
        .map(word => word.charAt(0).toUpperCase() + word.slice(1).toLowerCase())
        .join(" ");
}

/**
 * Try to extract artist from filename (common patterns)
 */
function extractArtist(fileName: string): string {
    // Pattern: "Artist - Title" or "Artist_Title"
    const patterns = [
        /^([^-\_]+)[\-\_](.+)$/,
    ];
    
    for (const pattern of patterns) {
        const match = fileName.match(pattern);
        if (match && match[1].trim().length > 0) {
            return match[1].trim();
        }
    }
    
    return "Unknown";
}

/**
 * Try to extract album from filename (less common pattern)
 */
function extractAlbum(fileName: string): string {
    // If we have "Artist - Album - Title" pattern
    const match = fileName.match(/^([^-\_]+)[\-\_]([^-\_]+)[\-\_](.+)$/);
    if (match && match[2].trim().length > 0) {
        return match[2].trim();
    }
    
    return "Unknown";
}

/**
 * Validate if a file is a supported audio format using the library
 */
export function isAudioFileSupported(filePath: string): boolean {
    const extension = filePath.split(".").pop()?.toLowerCase();
    if (!extension) return false;
    return isFormatSupported(extension);
}
