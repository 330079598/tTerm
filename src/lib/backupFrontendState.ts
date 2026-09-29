import { RECENT_COMMANDS_STORAGE_KEY } from "@/lib/recentCommands"

/** Data kept in localStorage that backups carry alongside the backend's. */
export interface BackupFrontendState {
  customThemes: unknown[]
  recentCommands: unknown[]
  sftpColumnWidths: unknown
}

export function readBackupFrontendState(): BackupFrontendState {
  const read = (key: string, fallback: unknown) => {
    try {
      return JSON.parse(localStorage.getItem(key) ?? JSON.stringify(fallback)) as unknown
    } catch {
      return fallback
    }
  }
  const customThemes = read("custom-themes", [])
  const recentCommands = read(RECENT_COMMANDS_STORAGE_KEY, [])
  return {
    customThemes: Array.isArray(customThemes) ? customThemes : [],
    recentCommands: Array.isArray(recentCommands) ? recentCommands : [],
    sftpColumnWidths: read("tterm.sftp.columnWidths", null),
  }
}
