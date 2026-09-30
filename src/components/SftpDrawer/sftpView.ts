import type { SftpDirectoryEntry } from "@/components/SftpDrawer/types"

/** Columns of the file table, in display order. */
export const SFTP_SORT_COLUMNS = [
  "name",
  "modified",
  "size",
  "kind",
  "permissions",
  "owner",
] as const

export type SftpSortColumn = (typeof SFTP_SORT_COLUMNS)[number]

export interface SftpSort {
  column: SftpSortColumn
  direction: "asc" | "desc"
}

/** How the listing is presented; remembered across sessions. */
export interface SftpViewPreferences {
  sort: SftpSort
  showHidden: boolean
}

export const DEFAULT_SFTP_VIEW_PREFERENCES: SftpViewPreferences = {
  sort: { column: "name", direction: "asc" },
  showHidden: true,
}

const SFTP_VIEW_STORAGE_KEY = "tterm.sftp.view"

const nameCollator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" })

export function isHiddenEntry(entry: Pick<SftpDirectoryEntry, "name">): boolean {
  return entry.name.startsWith(".")
}

/** Clicking the sorted column flips its direction; another column starts ascending. */
export function nextSftpSort(current: SftpSort, column: SftpSortColumn): SftpSort {
  return current.column === column
    ? { column, direction: current.direction === "asc" ? "desc" : "asc" }
    : { column, direction: "asc" }
}

/** File extension, lowercased; empty for folders, dotfiles and names without one. */
function extensionOf(entry: SftpDirectoryEntry): string {
  if (entry.isDir) return ""
  const dot = entry.name.lastIndexOf(".")
  return dot > 0 ? entry.name.slice(dot + 1).toLocaleLowerCase() : ""
}

function compareByColumn(
  left: SftpDirectoryEntry,
  right: SftpDirectoryEntry,
  column: SftpSortColumn
): number {
  switch (column) {
    case "name":
      return 0
    case "modified":
      return (left.modifiedAt ?? 0) - (right.modifiedAt ?? 0)
    case "size":
      return (left.size ?? 0) - (right.size ?? 0)
    case "kind":
      return (
        Number(left.isSymlink) - Number(right.isSymlink) ||
        extensionOf(left).localeCompare(extensionOf(right))
      )
    case "permissions":
      return (left.permissions ?? "").localeCompare(right.permissions ?? "")
    case "owner":
      return (
        (left.owner ?? "").localeCompare(right.owner ?? "") ||
        (left.group ?? "").localeCompare(right.group ?? "")
      )
  }
}

/**
 * A sorted copy of `entries`. Folders stay ahead of files whatever the
 * direction, and equal values fall back to the name so the order is stable.
 */
export function sortSftpEntries(
  entries: SftpDirectoryEntry[],
  sort: SftpSort
): SftpDirectoryEntry[] {
  const sign = sort.direction === "asc" ? 1 : -1
  return [...entries].sort(
    (left, right) =>
      Number(right.isDir) - Number(left.isDir) ||
      sign * compareByColumn(left, right, sort.column) ||
      sign * nameCollator.compare(left.name, right.name)
  )
}

/** The entries to show for `preferences`, in display order. */
export function applySftpView(
  entries: SftpDirectoryEntry[],
  preferences: SftpViewPreferences
): SftpDirectoryEntry[] {
  return sortSftpEntries(
    preferences.showHidden ? entries : entries.filter((entry) => !isHiddenEntry(entry)),
    preferences.sort
  )
}

export function loadSftpViewPreferences(
  storage: Pick<Storage, "getItem"> = window.localStorage
): SftpViewPreferences {
  try {
    const stored: unknown = JSON.parse(storage.getItem(SFTP_VIEW_STORAGE_KEY) ?? "null")
    if (!stored || typeof stored !== "object") return DEFAULT_SFTP_VIEW_PREFERENCES
    const { sort, showHidden } = stored as Partial<SftpViewPreferences>
    return {
      sort:
        sort &&
        SFTP_SORT_COLUMNS.includes(sort.column) &&
        (sort.direction === "asc" || sort.direction === "desc")
          ? { column: sort.column, direction: sort.direction }
          : DEFAULT_SFTP_VIEW_PREFERENCES.sort,
      showHidden:
        typeof showHidden === "boolean" ? showHidden : DEFAULT_SFTP_VIEW_PREFERENCES.showHidden,
    }
  } catch {
    return DEFAULT_SFTP_VIEW_PREFERENCES
  }
}

export function saveSftpViewPreferences(
  preferences: SftpViewPreferences,
  storage: Pick<Storage, "setItem"> = window.localStorage
) {
  try {
    storage.setItem(SFTP_VIEW_STORAGE_KEY, JSON.stringify(preferences))
  } catch (error) {
    console.error("Failed to persist SFTP view preferences:", error)
  }
}

/** The nine rwx bits of a mode, as the permissions dialog shows them. */
export const PERMISSION_BITS = [
  { scope: "owner", read: 0o400, write: 0o200, execute: 0o100 },
  { scope: "group", read: 0o040, write: 0o020, execute: 0o010 },
  { scope: "others", read: 0o004, write: 0o002, execute: 0o001 },
] as const

/** Three or four octal digits, e.g. `644` or `2755`. */
export function parseOctalMode(value: string): number | null {
  const trimmed = value.trim()
  return /^[0-7]{3,4}$/.test(trimmed) ? Number.parseInt(trimmed, 8) : null
}

export function formatOctalMode(mode: number): string {
  return (mode & 0o7777).toString(8).padStart(3, "0")
}
