import { PROFILE_RECENTS_STORAGE_KEY } from "@/lib/profileRecents"
import { QUICK_CONNECT_RECENTS_STORAGE_KEY } from "@/lib/quickConnect"
import { RECENT_COMMANDS_STORAGE_KEY } from "@/lib/recentCommands"
import { SFTP_VIEW_STORAGE_KEY } from "@/components/SftpDrawer/sftpView"
import type { BackupSelection } from "@/components/SettingsDialog/backupShared"

interface CategoryDiff {
  added: number
  updated: number
  unchanged: number
}

interface BackupManifest {
  appVersion: string
  createdAt: string
  platform: string
  selection: BackupSelection
  encrypted: boolean
  secretCount: number
}

export interface BackupInspectResult {
  manifest: BackupManifest
  requiresPassword: boolean
  passwordVerified: boolean
  webdavPasswordUsed: boolean
  profileCount: number
  commandCount: number
  secretCount: number
  hasFrontendState: boolean
  logFileCount: number
  diff: {
    profiles: CategoryDiff
    commands: CategoryDiff
    settingsChanged: boolean
    webdavAccount: { from: string | null; to: string } | null
  }
}

export interface AutomaticBackupSettings {
  frequency: "off" | "daily" | "weekly"
  directory: string
  retentionCount: number
  selection: BackupSelection
  /** When the last automatic backup ran; absent before the first. */
  lastBackupAt?: number | null
}

export interface BackupHistoryEntry {
  path: string
  fileName: string
  sizeBytes: number
  modifiedAt: number
  kind: "automatic" | "recovery" | "manual"
}

export interface BackupExportResult {
  outputPath: string
  profileCount: number
  commandCount: number
  secretCount: number
  encrypted: boolean
}

export interface BackupImportResult {
  profilesImported: number
  commandsImported: number
  secretsImported: number
  frontendState: {
    recentCommands?: unknown[]
    recentQuickConnections?: unknown[]
    recentProfiles?: unknown[]
    sftpColumnWidths?: unknown
    sftpView?: unknown
  } | null
  preImportBackupPath: string
  requiresRestart: boolean
  /** Key files imported profiles use that are not on this device. */
  missingKeyFiles: { path: string; profiles: string[] }[]
}

export type MigrationView = "backup" | "webdav" | "import" | "history"

export const defaultSelection: BackupSelection = {
  settings: true,
  profiles: true,
  session: true,
  knownHosts: true,
  sftpDirectories: true,
  commandLibrary: true,
  themes: true,
  secrets: false,
  logs: false,
}

export function cloneAvailableSelection(selection: BackupSelection): BackupSelection {
  return { ...selection }
}

/** Restores what the web view keeps; the backend restored the themes. */
export function restoreFrontendState(state: BackupImportResult["frontendState"]) {
  if (state && Array.isArray(state.recentCommands)) {
    localStorage.setItem(RECENT_COMMANDS_STORAGE_KEY, JSON.stringify(state.recentCommands))
  }
  if (state && Array.isArray(state.recentQuickConnections)) {
    localStorage.setItem(
      QUICK_CONNECT_RECENTS_STORAGE_KEY,
      JSON.stringify(state.recentQuickConnections)
    )
  }
  if (state && Array.isArray(state.recentProfiles)) {
    localStorage.setItem(PROFILE_RECENTS_STORAGE_KEY, JSON.stringify(state.recentProfiles))
  }
  if (state && state.sftpColumnWidths !== undefined && state.sftpColumnWidths !== null) {
    localStorage.setItem("tterm.sftp.columnWidths", JSON.stringify(state.sftpColumnWidths))
  }
  if (state && state.sftpView !== undefined && state.sftpView !== null) {
    localStorage.setItem(SFTP_VIEW_STORAGE_KEY, JSON.stringify(state.sftpView))
  }
}

export type BusyAction =
  "export" | "inspect" | "verify" | "import" | "saveSettings" | "backupNow" | "deleteHistory"
