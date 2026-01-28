import { unlink } from "node:fs/promises";
import path from "path";

export class JSONStorage {
  private readonly storageDir: string;

  constructor(folderName: string = "data") {
    this.storageDir = path.resolve(process.cwd(), folderName);
  }

  private getPath(key: string): string {
    return path.join(this.storageDir, key.endsWith(".json") ? key : `${key}.json`);
  }

  /**
   * Saves data to a JSON file.
   */
  async set(key: string, value: unknown): Promise<void> {
    const filePath = this.getPath(key);
    const content = JSON.stringify(value, null, 2);
    
    // Bun.write handles directory creation automatically in most environments
    await Bun.write(filePath, content);
  }

  /**
   * Retrieves and parses data. Returns null if not found.
   */
  async get<T>(key: string): Promise<T | null> {
    const file = Bun.file(this.getPath(key));

    if (!(await file.exists())) {
      return null;
    }

    return await file.json();
  }

  /**
   * Checks if a key exists.
   */
  async has(key: string): Promise<boolean> {
    return await Bun.file(this.getPath(key)).exists();
  }

  /**
   * Removes a file/key from storage.
   */
  async remove(key: string): Promise<void> {
    try {
      await unlink(this.getPath(key));
    } catch (e: any) {
      if (e.code !== "ENOENT") throw e;
    }
  }

  /**
   * Clears all files in the storage directory.
   */
  async clear(): Promise<void> {
    const { rm } = await import("node:fs/promises");
    await rm(this.storageDir, { recursive: true, force: true });
  }
}

// Export a default instance for easy use
export const storage = new JSONStorage();