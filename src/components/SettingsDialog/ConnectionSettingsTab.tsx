import React from "react"
import { DragDropProvider } from "@dnd-kit/react"
import { useSortable } from "@dnd-kit/react/sortable"
import {
  Activity,
  Clock3,
  Cpu,
  Gauge,
  GripVertical,
  HardDrive,
  type LucideIcon,
  MemoryStick,
  Network,
  RotateCcw,
  Route,
  Waves,
} from "lucide-react"
import { useTranslation } from "react-i18next"

import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Label } from "@/components/ui/label"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Switch } from "@/components/ui/switch"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { SettingsRow, SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import { DEFAULT_MONITOR_VISIBLE_METRICS, type MonitorMetricId } from "@/contexts/ConfigContext"
import { cn } from "@/lib/utils"

const monitorMetricDragId = (id: MonitorMetricId) => `monitor-metric:${id}:drag`
const getMonitorMetricId = (id?: string | number | null): MonitorMetricId | null => {
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

interface MonitorMetricRowProps {
  checked: boolean
  icon: LucideIcon
  id: MonitorMetricId
  index: number
  isLastSelected: boolean
  label: string
  onCheckedChange: (checked: boolean) => void
}

const MonitorMetricRow: React.FC<MonitorMetricRowProps> = ({
  checked,
  icon: Icon,
  id,
  index,
  isLastSelected,
  label,
  onCheckedChange,
}) => {
  const { t } = useTranslation()
  const { ref, handleRef, isDragging, isDropTarget } = useSortable({
    id: monitorMetricDragId(id),
    index,
    group: "monitor-status-metrics",
    disabled: !checked,
    transition: {
      duration: 180,
      easing: "cubic-bezier(0.2, 0.8, 0.2, 1)",
    },
  })
  const dragLabel = t("settings.monitorMetricDragLabel", {
    metric: label,
    defaultValue: `Drag to reorder ${label}`,
  })

  return (
    <div
      ref={ref}
      className={cn(
        "border-border bg-background flex min-h-10 items-center gap-2 rounded-md border px-2 py-2 transition-colors",
        checked ? "hover:bg-accent/35 will-change-transform" : "opacity-70",
        isDragging && "z-10 opacity-80 shadow-md",
        isDropTarget && "border-primary/70 bg-primary/5"
      )}
      role="listitem"
    >
      {checked ? (
        <Tooltip>
          <TooltipTrigger asChild>
            <button
              ref={handleRef}
              type="button"
              className="text-muted-foreground hover:bg-accent hover:text-foreground focus-visible:ring-ring/50 flex size-7 shrink-0 cursor-grab touch-none items-center justify-center rounded-sm outline-none focus-visible:ring-[3px] active:cursor-grabbing"
              aria-label={dragLabel}
            >
              <GripVertical size={15} aria-hidden="true" />
            </button>
          </TooltipTrigger>
          <TooltipContent>{dragLabel}</TooltipContent>
        </Tooltip>
      ) : (
        <span className="size-7 shrink-0" aria-hidden="true" />
      )}
      <Checkbox
        checked={checked}
        disabled={isLastSelected}
        onCheckedChange={onCheckedChange}
        aria-label={label}
      />
      <Icon size={15} className="text-muted-foreground shrink-0" aria-hidden="true" />
      <button
        type="button"
        className="min-w-0 flex-1 cursor-pointer truncate text-left text-sm"
        disabled={isLastSelected}
        onClick={() => onCheckedChange(!checked)}
      >
        {label}
      </button>
    </div>
  )
}

interface ConnectionSettingsTabProps {
  handleShowJumpHostConnectionInfoChange: (checked: boolean) => Promise<void>
  handleMonitorRefreshIntervalChange: (seconds: number) => Promise<void>
  handleMonitorVisibleMetricsChange: (metrics: MonitorMetricId[]) => Promise<void>
  monitorRefreshIntervalSecs: number
  monitorVisibleMetrics: MonitorMetricId[]
  showJumpHostConnectionInfo: boolean
}

export const ConnectionSettingsTab: React.FC<ConnectionSettingsTabProps> = ({
  handleShowJumpHostConnectionInfoChange,
  handleMonitorRefreshIntervalChange,
  handleMonitorVisibleMetricsChange,
  monitorRefreshIntervalSecs,
  monitorVisibleMetrics,
  showJumpHostConnectionInfo,
}) => {
  const { t } = useTranslation()
  const [monitorRefreshDraft, setMonitorRefreshDraft] = React.useState(
    String(monitorRefreshIntervalSecs)
  )
  const [monitorVisibleMetricsDraft, setMonitorVisibleMetricsDraft] =
    React.useState(monitorVisibleMetrics)

  React.useEffect(() => {
    setMonitorRefreshDraft(String(monitorRefreshIntervalSecs))
  }, [monitorRefreshIntervalSecs])

  React.useEffect(() => {
    setMonitorVisibleMetricsDraft(monitorVisibleMetrics)
  }, [monitorVisibleMetrics])

  const commitMonitorRefreshInterval = React.useCallback(() => {
    const value = parseInt(monitorRefreshDraft, 10)
    if (Number.isNaN(value)) {
      setMonitorRefreshDraft(String(monitorRefreshIntervalSecs))
      return
    }

    const normalizedValue = Math.min(Math.max(value, 1), 60)
    setMonitorRefreshDraft(String(normalizedValue))
    if (normalizedValue !== monitorRefreshIntervalSecs) {
      void handleMonitorRefreshIntervalChange(normalizedValue)
    }
  }, [handleMonitorRefreshIntervalChange, monitorRefreshDraft, monitorRefreshIntervalSecs])

  const monitorMetrics = React.useMemo(
    () => [
      { id: "cpu" as const, icon: Cpu, label: t("settings.monitorMetrics.cpu") },
      { id: "memory" as const, icon: MemoryStick, label: t("settings.monitorMetrics.memory") },
      { id: "network" as const, icon: Network, label: t("settings.monitorMetrics.network") },
      { id: "ip" as const, icon: Network, label: t("settings.monitorMetrics.ip") },
      { id: "latency" as const, icon: Gauge, label: t("settings.monitorMetrics.latency") },
      { id: "disk" as const, icon: HardDrive, label: t("settings.monitorMetrics.disk") },
      { id: "load" as const, icon: Waves, label: t("settings.monitorMetrics.load") },
      { id: "uptime" as const, icon: Clock3, label: t("settings.monitorMetrics.uptime") },
    ],
    [t]
  )

  const toggleMonitorMetric = React.useCallback(
    (metric: MonitorMetricId, checked: boolean) => {
      const nextMetrics = checked
        ? [...monitorVisibleMetricsDraft, metric]
        : monitorVisibleMetricsDraft.filter((id) => id !== metric)
      if (nextMetrics.length > 0) {
        setMonitorVisibleMetricsDraft(nextMetrics)
        void handleMonitorVisibleMetricsChange(nextMetrics)
      }
    },
    [handleMonitorVisibleMetricsChange, monitorVisibleMetricsDraft]
  )

  const orderedMonitorMetrics = React.useMemo(() => {
    const metricsById = new Map(monitorMetrics.map((metric) => [metric.id, metric]))
    return [
      ...monitorVisibleMetricsDraft.map((id) => metricsById.get(id)).filter(Boolean),
      ...monitorMetrics.filter(({ id }) => !monitorVisibleMetricsDraft.includes(id)),
    ] as typeof monitorMetrics
  }, [monitorMetrics, monitorVisibleMetricsDraft])

  const handleMonitorMetricDragEnd = React.useCallback(
    (
      event: Parameters<NonNullable<React.ComponentProps<typeof DragDropProvider>["onDragEnd"]>>[0]
    ) => {
      if (event.canceled) return

      const source = getMonitorMetricId(event.operation.source?.id ?? null)
      const target = getMonitorMetricId(event.operation.target?.id ?? null)
      if (!source || !target) return

      const reordered = reorderMonitorMetrics(monitorVisibleMetricsDraft, source, target)
      if (reordered !== monitorVisibleMetricsDraft) {
        setMonitorVisibleMetricsDraft(reordered)
        void handleMonitorVisibleMetricsChange(reordered)
      }
    },
    [handleMonitorVisibleMetricsChange, monitorVisibleMetricsDraft]
  )

  return (
    <ScrollArea className="h-full pr-4">
      <div className="space-y-6">
        <SettingsSection
          icon={<Route size={16} />}
          title={t("settings.jumpHost", { defaultValue: "Jump hosts" })}
          description={t("settings.connectionDesc", {
            defaultValue: "Control connection-related prompts and route details.",
          })}
        >
          <SettingsRow
            icon={<Route size={16} />}
            title={t("settings.showJumpHostConnectionInfo")}
            description={t("settings.showJumpHostConnectionInfoDesc")}
            action={
              <Switch
                checked={showJumpHostConnectionInfo}
                onCheckedChange={handleShowJumpHostConnectionInfoChange}
              />
            }
          />
        </SettingsSection>

        <SettingsSection
          icon={<Activity size={16} />}
          title={t("settings.monitor", { defaultValue: "Monitor" })}
          description={t("settings.monitorDesc", {
            defaultValue: "Tune how often SSH server metrics refresh while the monitor is visible.",
          })}
        >
          <SettingsRow
            icon={<Activity size={16} />}
            title={t("settings.monitorRefreshInterval", {
              defaultValue: "Server monitor refresh interval",
            })}
            description={t("settings.monitorRefreshIntervalDesc", {
              defaultValue:
                "Choose a compact cadence for CPU, memory, IP and disk metrics. Shorter intervals feel more live but run more SSH queries.",
            })}
          >
            <div className="flex max-w-xs items-end gap-3">
              <div className="min-w-0 flex-1 space-y-1.5">
                <Label htmlFor="monitor-refresh-interval" className="text-muted-foreground text-xs">
                  {t("settings.monitorRefreshIntervalSeconds", { defaultValue: "Seconds" })}
                </Label>
                <Input
                  id="monitor-refresh-interval"
                  type="number"
                  min={1}
                  max={60}
                  step={1}
                  value={monitorRefreshDraft}
                  onChange={(event) => {
                    const nextValue = event.target.value
                    setMonitorRefreshDraft(nextValue)

                    const value = parseInt(nextValue, 10)
                    if (!Number.isNaN(value) && value >= 1 && value <= 60) {
                      void handleMonitorRefreshIntervalChange(value)
                    }
                  }}
                  onBlur={commitMonitorRefreshInterval}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") {
                      event.currentTarget.blur()
                    }
                  }}
                  className="h-8"
                />
              </div>
              <div className="text-muted-foreground pb-2 text-xs">
                {t("settings.monitorRefreshIntervalRange", { defaultValue: "1-60s" })}
              </div>
            </div>
          </SettingsRow>
          <SettingsRow
            icon={<Activity size={16} />}
            title={t("settings.monitorVisibleMetrics", {
              defaultValue: "Status bar metrics",
            })}
            description={t("settings.monitorVisibleMetricsDesc", {
              defaultValue:
                "Choose the metrics shown in the compact monitor bar and drag selected metrics to reorder them.",
            })}
          >
            <DragDropProvider onDragEnd={handleMonitorMetricDragEnd}>
              <div className="max-w-md space-y-2" role="list">
                {orderedMonitorMetrics.map(({ id, icon, label }, index) => {
                  const checked = monitorVisibleMetricsDraft.includes(id)
                  return (
                    <MonitorMetricRow
                      key={id}
                      checked={checked}
                      icon={icon}
                      id={id}
                      index={index}
                      isLastSelected={checked && monitorVisibleMetricsDraft.length === 1}
                      label={label}
                      onCheckedChange={(nextChecked) => toggleMonitorMetric(id, nextChecked)}
                    />
                  )
                })}
              </div>
            </DragDropProvider>
            <Button
              type="button"
              variant="ghost"
              size="xs"
              className="mt-3"
              onClick={() => {
                const defaultMetrics = [...DEFAULT_MONITOR_VISIBLE_METRICS]
                setMonitorVisibleMetricsDraft(defaultMetrics)
                void handleMonitorVisibleMetricsChange(defaultMetrics)
              }}
            >
              <RotateCcw aria-hidden="true" />
              {t("settings.monitorResetMetrics", { defaultValue: "Restore defaults" })}
            </Button>
          </SettingsRow>
        </SettingsSection>
      </div>
    </ScrollArea>
  )
}
