import { SFTP_VIEW_STORAGE_KEY } from "@/components/SftpDrawer/sftpView"
import { QUICK_CONNECT_RECENTS_STORAGE_KEY } from "@/lib/quickConnect"
import { RECENT_COMMANDS_STORAGE_KEY } from "@/lib/recentCommands"

/**
 * Data kept in localStorage that backups carry alongside the backend's. The
 * backend adds the custom themes, which it stores itself.
 */
export interface BackupFrontendState {
  recentCommands: unknown[]
  recentQuickConnections: unknown[]
  sftpColumnWidths: unknown
  sftpView: unknown
}

export function readBackupFrontendState(): BackupFrontendState {
  const read = (key: string, fallback: unknown) => {
    try {
      return JSON.parse(localStorage.getItem(key) ?? JSON.stringify(fallback)) as unknown
    } catch {
      return fallback
    }
  }
  const recentCommands = read(RECENT_COMMANDS_STORAGE_KEY, [])
  const recentQuickConnections = read(QUICK_CONNECT_RECENTS_STORAGE_KEY, [])
  return {
    recentCommands: Array.isArray(recentCommands) ? recentCommands : [],
    recentQuickConnections: Array.isArray(recentQuickConnections) ? recentQuickConnections : [],
    sftpColumnWidths: read("tterm.sftp.columnWidths", null),
    sftpView: read(SFTP_VIEW_STORAGE_KEY, null),
  }
}
