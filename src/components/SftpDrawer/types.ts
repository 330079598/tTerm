import type { Tab } from "@/types/tab"

export interface SftpDrawerProps {
  tabId: string
  visible: boolean
  isGlobalShortcutTarget: boolean
  connection?: Tab["connection"]
  onClose: () => void
  onOpenRemoteFile?: (
    entry: SftpDirectoryEntry,
    sourceTabId: string,
    connection?: Tab["connection"]
  ) => void
}

export interface SftpDirectoryEntry {
  name: string
  path: string
  isDir: boolean
  isSymlink: boolean
  size?: number
  modifiedAt?: number
  permissions?: string
  /** Permission bits without the file type, e.g. `0o644`. */
  mode?: number | null
  owner?: string
  group?: string
}

export interface SftpDirectoryListing {
  currentPath: string
  parentPath?: string | null
  entries: SftpDirectoryEntry[]
}

export type LoadSftpDirectory = (
  path?: string | null,
  options?: { throwOnError?: boolean }
) => Promise<void>

export interface SftpContextMenuState {
  x: number
  y: number
  entryPath: string
}

export type SftpDeleteMethod = "sftp" | "command"

export type SftpDialogState =
  | { type: "none" }
  | { type: "delete"; entries: SftpDirectoryEntry[] }
  | {
      type: "commandDelete"
      entries: SftpDirectoryEntry[]
      command: string
      totalDirectories: number
      totalEntries: number
      totalFiles: number
      totalTruncated: boolean
    }
  | { type: "rename"; entry: SftpDirectoryEntry | null; newName: string }
  | { type: "createFolder"; folderName: string }

export type SftpDialogAction =
  | { action: "close" }
  | { action: "openDelete"; entries: SftpDirectoryEntry[] }
  | {
      action: "openCommandDelete"
      entries: SftpDirectoryEntry[]
      command: string
      totalDirectories: number
      totalEntries: number
      totalFiles: number
      totalTruncated: boolean
    }
  | { action: "openRename"; entry: SftpDirectoryEntry; newName: string }
  | { action: "openCreateFolder" }
  | { action: "updateRenameNewName"; newName: string }
  | { action: "updateCreateFolderName"; folderName: string }
  | { action: "updateCommandDeleteCommand"; command: string }

export interface SftpDeleteProgressState {
  batchId: string
  currentPath: string
  deletedDirectories: number
  deletedFiles: number
  failed: number
  method: SftpDeleteMethod
  totalDirectories: number
  totalEntries: number
  totalFiles: number
  totalTruncated: boolean
}
export interface DeleteBatchStartResult {
  batchId: string
}

export interface DeletePreviewResult {
  command: string
  shouldPromptForCommand: boolean
  totalDirectories: number
  totalEntries: number
  totalFiles: number
  totalTruncated: boolean
}

export interface DeleteBatchStartEvent extends SftpDeleteProgressState {
  entries: string[]
}

export interface DeleteBatchCompleteEvent extends SftpDeleteProgressState {
  cancelled: boolean
  error?: string
}

/** How a transfer treats a destination file that already exists. */
export type ConflictPolicy = "overwrite" | "skip" | "overwriteIfNewer" | "rename"

export interface TransferConflict {
  sourcePath: string
  targetPath: string
  sourceSize: number
  /** Seconds since the Unix epoch. */
  sourceMtime?: number | null
  targetSize: number
  /** Seconds since the Unix epoch. */
  targetMtime?: number | null
  targetIsDir: boolean
}

export interface ConflictReport {
  /** Every conflicting file, including those beyond `conflicts`. */
  total: number
  /** Files the whole transfer would write. */
  fileCount: number
  /** A bounded prefix of the conflicts, in transfer order. */
  conflicts: TransferConflict[]
}

/**
 * Asks how to resolve the conflicts of a pending transfer. Resolves to the
 * chosen policy, or null when the user cancels the transfer.
 */
export type PromptConflictPolicy = (
  report: ConflictReport,
  direction: "upload" | "download"
) => Promise<ConflictPolicy | null>
