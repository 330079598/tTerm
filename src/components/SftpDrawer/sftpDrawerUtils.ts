export function joinRemotePath(basePath: string, name: string): string {
  if (basePath === "/") {
    return `/${name}`
  }
  return `${basePath}/${name}`
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
