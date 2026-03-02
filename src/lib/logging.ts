import { invoke } from '@tauri-apps/api/core';

export type LogLevel = 'error' | 'warn' | 'info' | 'debug';

export async function logTo(level: LogLevel, message: string): Promise<void> {
  try {
    await invoke('log_to_file', { level, message });
  } catch {
    // If logging fails, fall back to console
    const fn = level === 'error' ? console.error : level === 'warn' ? console.warn : console.log;
    fn(`[${level.toUpperCase()}] ${message}`);
  }
}

export async function logError(message: string): Promise<void> {
  return logTo('error', message);
}

export async function logWarn(message: string): Promise<void> {
  return logTo('warn', message);
}

export async function logInfo(message: string): Promise<void> {
  return logTo('info', message);
}

export async function logDebug(message: string): Promise<void> {
  return logTo('debug', message);
}
