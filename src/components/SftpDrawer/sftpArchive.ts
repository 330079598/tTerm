import { parentRemotePath } from "@/components/SftpDrawer/sftpDrawerUtils"
import type { SftpDirectoryEntry } from "@/components/SftpDrawer/types"

/** Mirrors `ArchiveFormat` in `src-tauri/src/sftp/internal/api/archive.rs`. */
export type ArchiveFormat =
  "zip" | "tar" | "tarGz" | "tarBz2" | "tarXz" | "tarZst" | "sevenZip" | "gzip" | "bzip2" | "xz"

/** The formats an archive can be made in, the most common first. */
export const COMPRESS_FORMATS = ["tarGz", "zip", "tarXz", "tarBz2", "tar"] as const
export type CompressFormat = (typeof COMPRESS_FORMATS)[number]

export const COMPRESS_EXTENSIONS: Record<CompressFormat, string> = {
  tarGz: ".tar.gz",
  zip: ".zip",
  tarXz: ".tar.xz",
  tarBz2: ".tar.bz2",
  tar: ".tar",
}

// Longer suffixes come first so `.tar.gz` wins over `.gz`.
const EXTRACT_SUFFIXES: Array<[string, ArchiveFormat]> = [
  [".tar.gz", "tarGz"],
  [".tgz", "tarGz"],
  [".tar.bz2", "tarBz2"],
  [".tbz2", "tarBz2"],
  [".tbz", "tarBz2"],
  [".tar.xz", "tarXz"],
  [".txz", "tarXz"],
  [".tar.zst", "tarZst"],
  [".tzst", "tarZst"],
  [".tar", "tar"],
  [".zip", "zip"],
  [".7z", "sevenZip"],
  [".gz", "gzip"],
  [".bz2", "bzip2"],
  [".xz", "xz"],
]

/** Formats that hold one compressed file rather than a tree of entries. */
const SINGLE_FILE_FORMATS = new Set<ArchiveFormat>(["gzip", "bzip2", "xz"])

function matchSuffix(name: string): [string, ArchiveFormat] | null {
  const lower = name.toLowerCase()
  return EXTRACT_SUFFIXES.find(([suffix]) => lower.endsWith(suffix)) ?? null
}

/** The format of an archive file, judged by its name; null for other files. */
export function archiveFormatOf(entry: Pick<SftpDirectoryEntry, "name" | "isDir">) {
  if (entry.isDir) return null
  const match = matchSuffix(entry.name)
  return match && match[0].length < entry.name.length ? match[1] : null
}

/**
 * Where an archive unpacks by default: a folder named after it beside it, or
 * the folder it is in for a single compressed file, which unpacks to one file.
 */
export function defaultExtractDestination(archivePath: string, format: ArchiveFormat) {
  const parent = parentRemotePath(archivePath)
  if (SINGLE_FILE_FORMATS.has(format)) return parent

  const name = archivePath.slice(archivePath.lastIndexOf("/") + 1)
  const match = matchSuffix(name)
  const stem = match ? name.slice(0, name.length - match[0].length) : name
  return parent === "/" ? `/${stem}` : `${parent}/${stem}`
}

/**
 * The name offered for a new archive: the entry's own name for one entry,
 * the folder's name for several.
 */
export function defaultArchiveName(
  entries: Array<Pick<SftpDirectoryEntry, "name">>,
  folderPath: string,
  format: CompressFormat
) {
  const folderName = folderPath.split("/").filter(Boolean).pop()
  const base = entries.length === 1 ? entries[0].name : (folderName ?? "archive")
  return `${base}${COMPRESS_EXTENSIONS[format]}`
}

/** Swaps the extension of `name` when it still carries the old format's. */
export function withArchiveExtension(name: string, from: CompressFormat, to: CompressFormat) {
  const oldExtension = COMPRESS_EXTENSIONS[from]
  if (!name.toLowerCase().endsWith(oldExtension)) return name
  return `${name.slice(0, name.length - oldExtension.length)}${COMPRESS_EXTENSIONS[to]}`
}

/** The folder holding all of `entries`, or null when they are in several. */
export function commonParent(entries: Array<Pick<SftpDirectoryEntry, "path">>) {
  const parents = new Set(entries.map((entry) => parentRemotePath(entry.path)))
  return parents.size === 1 ? [...parents][0] : null
}
