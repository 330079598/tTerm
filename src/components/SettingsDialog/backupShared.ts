export interface BackupSelection {
  settings: boolean
  profiles: boolean
  session: boolean
  knownHosts: boolean
  sftpDirectories: boolean
  commandLibrary: boolean
  themes: boolean
  secrets: boolean
  logs: boolean
}

export type SelectionKey = keyof BackupSelection

export interface SelectionItem {
  key: SelectionKey
  label: string
}

/** Checks a category, keeping passwords only together with their profiles. */
export function withSelection(
  selection: BackupSelection,
  key: SelectionKey,
  checked: boolean
): BackupSelection {
  const next = { ...selection, [key]: checked }
  if (key === "secrets" && checked) next.profiles = true
  if (key === "profiles" && !checked) next.secrets = false
  return next
}

export function formatFileSize(bytes: number) {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}
