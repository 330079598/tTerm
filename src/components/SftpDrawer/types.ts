import type { Tab } from "@/types/tab"

export interface SftpDrawerProps {
  tabId: string
  visible: boolean
  connection?: Tab["connection"]
  onClose: () => void
}

export interface SftpDirectoryEntry {
  name: string
  path: string
  isDir: boolean
  isSymlink: boolean
  size?: number
  modifiedAt?: number
  permissions?: string
  owner?: string
  group?: string
}

export interface SftpDirectoryListing {
  currentPath: string
  parentPath?: string | null
  entries: SftpDirectoryEntry[]
}
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
