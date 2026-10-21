import type { TransferTask } from "@/types/tab"

/** A command running on the server, which reports no progress until it ends. */
export function isRemoteOperation(transfer: Pick<TransferTask, "direction">) {
  return transfer.direction === "compress" || transfer.direction === "extract"
}

export function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B"
  const k = 1024
  const sizes = ["B", "KB", "MB", "GB"]
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`
}

export function formatSpeed(bytesPerSecond: number): string {
  return `${formatBytes(bytesPerSecond)}/s`
}

export function formatDuration(ms: number): string {
  const seconds = Math.floor(ms / 1000)
  if (seconds < 60) return `${seconds}s`
  const minutes = Math.floor(seconds / 60)
  const remainingSeconds = seconds % 60
  return `${minutes}m ${remainingSeconds}s`
}

type Translator = (key: string, options?: Record<string, unknown>) => string

export function formatItems(count: number, t: Translator): string {
  return t("transfer.itemCount", { count, defaultValue: `${count} item(s)` })
}

export function formatItemSpeed(itemsPerSecond: number, t: Translator): string {
  return `${formatItems(Math.round(itemsPerSecond), t)}/s`
}
