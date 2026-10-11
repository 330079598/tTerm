/** A figure the server monitor bar can show for an SSH host. */
export type MonitorMetricId =
  "cpu" | "memory" | "network" | "ip" | "latency" | "disk" | "load" | "uptime"

export const DEFAULT_MONITOR_VISIBLE_METRICS: MonitorMetricId[] = [
  "cpu",
  "memory",
  "network",
  "ip",
  "latency",
  "disk",
]

const MONITOR_METRIC_IDS = new Set<MonitorMetricId>([
  ...DEFAULT_MONITOR_VISIBLE_METRICS,
  "load",
  "uptime",
])

/** The saved metrics in their order, without unknown ids or repeats; the defaults when none is left. */
export function normalizeMonitorVisibleMetrics(value: unknown): MonitorMetricId[] {
  if (!Array.isArray(value)) {
    return [...DEFAULT_MONITOR_VISIBLE_METRICS]
  }

  const metrics = value.filter(
    (item, index): item is MonitorMetricId =>
      MONITOR_METRIC_IDS.has(item as MonitorMetricId) && value.indexOf(item) === index
  )
  return metrics.length > 0 ? metrics : [...DEFAULT_MONITOR_VISIBLE_METRICS]
}
