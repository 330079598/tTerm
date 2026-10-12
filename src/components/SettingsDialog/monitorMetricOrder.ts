import type { MonitorMetricId } from "@/lib/monitorMetrics"

export const monitorMetricDragId = (id: MonitorMetricId) => `monitor-metric:${id}:drag`
export const getMonitorMetricId = (id?: string | number | null): MonitorMetricId | null => {
  if (!id) return null
  const parts = String(id).split(":")
  if (parts.length !== 3 || parts[0] !== "monitor-metric") return null
  return parts[1] as MonitorMetricId
}

export function reorderMonitorMetrics(
  metrics: MonitorMetricId[],
  source: MonitorMetricId,
  target: MonitorMetricId
): MonitorMetricId[] {
  const sourceIndex = metrics.indexOf(source)
  const targetIndex = metrics.indexOf(target)
  if (sourceIndex < 0 || targetIndex < 0 || sourceIndex === targetIndex) {
    return metrics
  }

  const reordered = [...metrics]
  const [moved] = reordered.splice(sourceIndex, 1)
  reordered.splice(targetIndex, 0, moved)
  return reordered
}
