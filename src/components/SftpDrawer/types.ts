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

export interface SftpDeleteDialogState {
  open: boolean
  entries: SftpDirectoryEntry[]
}

export interface SftpRenameDialogState {
  open: boolean
  entry: SftpDirectoryEntry | null
  newName: string
}

export interface SftpCreateFolderDialogState {
  open: boolean
  folderName: string
}

export type SftpDeleteMethod = "sftp" | "command"

export interface SftpDeleteProgressState {
  batchId: string
  currentPath: string
  deletedDirectories: number
  deletedFiles: number
  failed: number
  method: SftpDeleteMethod
  totalDirectories: number
  totalFiles: number
  totalTruncated: boolean
}
