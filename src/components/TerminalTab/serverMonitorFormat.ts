import type { ServerMetricsSnapshot } from "@/components/TerminalTab/serverMetrics"

export function formatKib(value?: number) {
  if (value === undefined) return "--"
  const bytes = value * 1024
  const units = ["B", "KB", "MB", "GB", "TB"]
  let current = bytes
  let unitIndex = 0
  while (current >= 1024 && unitIndex < units.length - 1) {
    current /= 1024
    unitIndex += 1
  }
  const digits = current >= 10 || unitIndex < 2 ? 0 : 1
  return `${current.toFixed(digits)}${units[unitIndex]}`
}

export function formatKibAsG(value?: number) {
  if (value === undefined) return "--"
  const gib = value / 1024 / 1024
  const digits = gib >= 10 ? 0 : 1
  return `${gib.toFixed(digits)}G`
}

export function severityClass(percent?: number) {
  if (percent === undefined) return "is-muted"
  if (percent >= 90) return "is-critical"
  if (percent >= 70) return "is-warning"
  return "is-normal"
}

export function latencySeverityClass(latencyMs?: number) {
  if (latencyMs === undefined) return "is-muted"
  if (latencyMs >= 300) return "is-critical"
  if (latencyMs >= 150) return "is-warning"
  return "is-normal"
}

export function formatLatency(latencyMs?: number) {
  if (latencyMs === undefined) return "--"
  return latencyMs === 0 ? "<1 ms" : `${latencyMs} ms`
}

export function formatNetworkRate(value?: number) {
  if (value === undefined || !Number.isFinite(value)) return "--"
  const units = ["B", "K", "M", "G"]
  let current = Math.max(0, value)
  let index = 0
  while (current >= 1024 && index < units.length - 1) {
    current /= 1024
    index += 1
  }
  const digits = current >= 100 || index === 0 ? 0 : current >= 10 ? 1 : 2
  return `${current.toFixed(digits)}${units[index]}/s`
}

export function formatUptime(seconds?: number) {
  if (seconds === undefined) return "--"
  const days = Math.floor(seconds / 86400)
  const hours = Math.floor((seconds % 86400) / 3600)
  return days > 0 ? `${days}d ${hours}h` : `${hours}h`
}

export function getDistroLabel(snapshot?: ServerMetricsSnapshot) {
  if (!snapshot?.distribution) return "Linux"
  const pretty = snapshot.distribution.prettyName || snapshot.distribution.name
  return pretty || "Linux"
}
