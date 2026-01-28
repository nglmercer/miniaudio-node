export class Logger {
  private static readonly levelMap = {
    trace: 0,
    debug: 1,
    info: 2,
    warn: 3,
    error: 4,
    off: 5,
  } as const;

  private static currentLevel: number = Logger.levelMap.info;

  /**
   * Sets the global logging level.
   * Logs with priority lower than this level will be ignored.
   */
  public static setLevel(level: keyof typeof Logger.levelMap): void {
    Logger.currentLevel = Logger.levelMap[level];
  }

  private static shouldLog(level: number): boolean {
    // Logic fix: Only log if the message priority is >= the current threshold
    return level >= Logger.currentLevel && Logger.currentLevel !== Logger.levelMap.off;
  }

  private static format(level: string): string {
    const timestamp = new Date().toISOString();
    return `[${timestamp}] [${level.toUpperCase()}]`;
  }

  public static trace(message: string, ...args: any[]): void {
    if (this.shouldLog(Logger.levelMap.trace)) {
      console.trace(this.format('trace'), message, ...args);
    }
  }

  public static debug(message: string, ...args: any[]): void {
    if (this.shouldLog(Logger.levelMap.debug)) {
      console.debug(this.format('debug'), message, ...args);
    }
  }

  public static info(message: string, ...args: any[]): void {
    if (this.shouldLog(Logger.levelMap.info)) {
      console.info(this.format('info'), message, ...args);
    }
  }

  public static warn(message: string, ...args: any[]): void {
    if (this.shouldLog(Logger.levelMap.warn)) {
      console.warn(this.format('warn'), message, ...args);
    }
  }

  public static error(message: string, ...args: any[]): void {
    if (this.shouldLog(Logger.levelMap.error)) {
      console.error(this.format('error'), message, ...args);
    }
  }
}
export default Logger;