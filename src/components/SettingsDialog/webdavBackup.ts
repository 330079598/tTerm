import type { BackupSelection } from "@/components/SettingsDialog/backupShared"

export interface WebDavBackupSettings {
  url: string
  username: string
  remoteDirectory: string
  frequency: "off" | "daily" | "weekly"
  retentionCount: number
  selection: BackupSelection
  /** Upload history, sent by the backend and ignored when saved. */
  lastBackupAt?: number | null
  lastError?: string | null
}

export interface WebDavBackupStatus {
  settings: WebDavBackupSettings
  hasPassword: boolean
  hasBackupPassword: boolean
  deviceName: string
}

export interface WebDavUploadResult {
  fileName: string
  sizeBytes: number
  profileCount: number
  commandCount: number
  secretCount: number
  pruneError: string | null
}

export interface WebDavDeleteResult {
  deleted: string[]
  error: string | null
}

export interface RemoteBackupEntry {
  fileName: string
  sizeBytes: number
  createdAt: number | null
  device: string | null
  currentDevice: boolean
}

export type BusyAction = "save" | "test" | "upload" | "list" | "download" | "delete" | "clear"

/**
 * Where uploads go: the folder's segments appended to the server URL, the
 * way the backend joins them, so a leading "/" stays under the server path.
 */
export function backupLocation(serverUrl: string, directory: string): string | null {
  let url: URL
  try {
    url = new URL(serverUrl.trim())
  } catch {
    return null
  }
  if ((url.protocol !== "http:" && url.protocol !== "https:") || !url.hostname) return null
  const segments = directory
    .split("/")
    .map((segment) => segment.trim())
    .filter(Boolean)
  if (segments.some((segment) => segment === "." || segment === ".." || segment.includes("\\"))) {
    return null
  }
  const basePath = url.pathname.endsWith("/") ? url.pathname.slice(0, -1) : url.pathname
  let path = [basePath, ...segments].join("/") + "/"
  try {
    path = decodeURI(path)
  } catch {
    // Shown as typed when it is not valid percent-encoding.
  }
  return `${url.origin}${path}`
}
