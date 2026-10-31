import type React from "react"

export const MAX_EDIT_FILE_BYTES = 5 * 1024 * 1024

export function joinRemotePath(basePath: string, name: string): string {
  if (basePath === "/") {
    return `/${name}`
  }
  return `${basePath}/${name}`
}

/** The folder holding `path`; `/` for an entry of the root. */
export function parentRemotePath(path: string): string {
  const slash = path.lastIndexOf("/")
  return slash <= 0 ? "/" : path.slice(0, slash)
}

/**
 * Whether the server refused a request because the account may not do it, as
 * SFTP reports it ("Permission denied"): the error that working as root
 * through sudo can get past.
 */
export function isPermissionDenied(error: string | null): boolean {
  return error !== null && /permission denied/i.test(error)
}

/** The drawer's font sizes, which follow the terminal's. */
export function sftpFontSizeStyle(fontSize: number): React.CSSProperties {
  return {
    "--sftp-font-size": `calc(${Math.max(fontSize - 2, 10)}px * var(--ui-font-scale, 1))`,
    "--sftp-font-size-small": `calc(${Math.max(fontSize - 3, 9)}px * var(--ui-font-scale, 1))`,
    "--sftp-font-size-large": `calc(${Math.max(fontSize, 12)}px * var(--ui-font-scale, 1))`,
    "--sftp-font-size-title": `calc(${Math.max(fontSize + 4, 16)}px * var(--ui-font-scale, 1))`,
  } as React.CSSProperties
}

export function formatBytes(value?: number): string {
  if (value == null) return "--"
  if (value < 1024) return `${value} B`

  const units = ["KB", "MB", "GB", "TB"]
  let size = value / 1024
  let index = 0
  while (size >= 1024 && index < units.length - 1) {
    size /= 1024
    index += 1
  }
  return `${size.toFixed(size >= 10 ? 0 : 1)} ${units[index]}`
}

export function formatTimestamp(value?: number): string {
  if (!value) return "--"
  const date = new Date(value)
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, "0")
  const day = String(date.getDate()).padStart(2, "0")
  const hours = String(date.getHours()).padStart(2, "0")
  const minutes = String(date.getMinutes()).padStart(2, "0")
  const seconds = String(date.getSeconds()).padStart(2, "0")
  return `${year}-${month}-${day} ${hours}:${minutes}:${seconds}`
}
