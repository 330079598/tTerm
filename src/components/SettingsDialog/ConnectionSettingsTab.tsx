import React from "react"
import { DragDropProvider } from "@dnd-kit/react"
import {
  Activity,
  Asterisk,
  Clock3,
  Cpu,
  Gauge,
  HardDrive,
  KeyRound,
  MemoryStick,
  Network,
  RefreshCw,
  RotateCcw,
  Route,
  Waves,
} from "lucide-react"
import { useTranslation } from "react-i18next"

import {
  getMonitorMetricId,
  reorderMonitorMetrics,
} from "@/components/SettingsDialog/monitorMetricOrder"
import { MonitorMetricRow } from "@/components/SettingsDialog/MonitorMetricRow"
import { PredictiveEchoSection } from "@/components/SettingsDialog/PredictiveEchoSection"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import { Label } from "@/components/ui/label"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Switch } from "@/components/ui/switch"
import { SettingsRow, SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import { SudoPromptPatternsSetting } from "@/components/SettingsDialog/SudoPromptPatternsSetting"
import { RECONNECT_MAX_ATTEMPTS_MAX, RECONNECT_MAX_ATTEMPTS_MIN } from "@/lib/appConfig"
import { DEFAULT_MONITOR_VISIBLE_METRICS, type MonitorMetricId } from "@/lib/monitorMetrics"
import { useSyncedState } from "@/hooks/useSyncedState"
import { isImeKeyEvent } from "@/lib/ime"

interface ConnectionSettingsTabProps {
  handleReconnectEnabledChange: (checked: boolean) => Promise<void>
  handleReconnectMaxAttemptsChange: (attempts: number) => Promise<void>
  handleShowJumpHostConnectionInfoChange: (checked: boolean) => Promise<void>
  handleMonitorRefreshIntervalChange: (seconds: number) => Promise<void>
  handleMonitorVisibleMetricsChange: (metrics: MonitorMetricId[]) => Promise<void>
  handleSudoPromptPatternsChange: (patterns: string[]) => Promise<void>
  handlePasswordFeedbackChange: (checked: boolean) => Promise<void>
  monitorRefreshIntervalSecs: number
  monitorVisibleMetrics: MonitorMetricId[]
  passwordFeedback: boolean
  reconnectEnabled: boolean
  reconnectMaxAttempts: number
  showJumpHostConnectionInfo: boolean
  sudoPromptPatterns: string[]
}

export const ConnectionSettingsTab: React.FC<ConnectionSettingsTabProps> = ({
  handleReconnectEnabledChange,
  handleReconnectMaxAttemptsChange,
  handleShowJumpHostConnectionInfoChange,
  handleMonitorRefreshIntervalChange,
  handleMonitorVisibleMetricsChange,
  handleSudoPromptPatternsChange,
  handlePasswordFeedbackChange,
  monitorRefreshIntervalSecs,
  monitorVisibleMetrics,
  passwordFeedback,
  reconnectEnabled,
  reconnectMaxAttempts,
  showJumpHostConnectionInfo,
  sudoPromptPatterns,
}) => {
  const { t } = useTranslation()
  const [monitorRefreshDraft, setMonitorRefreshDraft] = useSyncedState(
    String(monitorRefreshIntervalSecs)
  )
  const [reconnectMaxAttemptsDraft, setReconnectMaxAttemptsDraft] = useSyncedState(
    String(reconnectMaxAttempts)
  )
  const [monitorVisibleMetricsDraft, setMonitorVisibleMetricsDraft] =
    useSyncedState(monitorVisibleMetrics)

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
  }, [
    handleMonitorRefreshIntervalChange,
    monitorRefreshDraft,
    monitorRefreshIntervalSecs,
    setMonitorRefreshDraft,
  ])

  const commitReconnectMaxAttempts = React.useCallback(() => {
    const value = parseInt(reconnectMaxAttemptsDraft, 10)
    if (Number.isNaN(value)) {
      setReconnectMaxAttemptsDraft(String(reconnectMaxAttempts))
      return
    }

    const normalizedValue = Math.min(
      Math.max(value, RECONNECT_MAX_ATTEMPTS_MIN),
      RECONNECT_MAX_ATTEMPTS_MAX
    )
    setReconnectMaxAttemptsDraft(String(normalizedValue))
    if (normalizedValue !== reconnectMaxAttempts) {
      void handleReconnectMaxAttemptsChange(normalizedValue)
    }
  }, [
    handleReconnectMaxAttemptsChange,
    reconnectMaxAttempts,
    reconnectMaxAttemptsDraft,
    setReconnectMaxAttemptsDraft,
  ])

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
    [handleMonitorVisibleMetricsChange, monitorVisibleMetricsDraft, setMonitorVisibleMetricsDraft]
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
    [handleMonitorVisibleMetricsChange, monitorVisibleMetricsDraft, setMonitorVisibleMetricsDraft]
  )

  return (
    <ScrollArea className="h-full pr-4">
      <div className="space-y-6">
        <SettingsSection
          icon={<RefreshCw size={16} />}
          title={t("settings.reconnect", { defaultValue: "Connection recovery" })}
          description={t("settings.reconnectDesc", {
            defaultValue: "Automatically re-establish SSH sessions that drop unexpectedly.",
          })}
        >
          <SettingsRow
            icon={<RefreshCw size={16} />}
            title={t("settings.autoReconnect", { defaultValue: "Automatically reconnect" })}
            description={t("settings.autoReconnectDesc", {
              defaultValue:
                "Retry a dropped SSH session with capped backoff up to the configured attempt limit. When off, the disconnect is reported instead.",
            })}
            action={
              <Switch checked={reconnectEnabled} onCheckedChange={handleReconnectEnabledChange} />
            }
          />
          <SettingsRow
            icon={<RefreshCw size={16} />}
            title={t("settings.reconnectMaxAttempts", {
              defaultValue: "Reconnect attempt limit",
            })}
            description={t("settings.reconnectMaxAttemptsDesc", {
              defaultValue:
                "Maximum automatic reconnect attempts per dropped session before the disconnect is reported. Defaults to 5.",
            })}
          >
            <div className="flex max-w-xs items-end gap-3">
              <div className="min-w-0 flex-1 space-y-1.5">
                <Label htmlFor="reconnect-max-attempts" className="text-muted-foreground text-xs">
                  {t("settings.reconnectMaxAttemptsTimes", { defaultValue: "Times" })}
                </Label>
                <Input
                  id="reconnect-max-attempts"
                  type="number"
                  min={RECONNECT_MAX_ATTEMPTS_MIN}
                  max={RECONNECT_MAX_ATTEMPTS_MAX}
                  step={1}
                  disabled={!reconnectEnabled}
                  value={reconnectMaxAttemptsDraft}
                  onChange={(event) => {
                    const nextValue = event.target.value
                    setReconnectMaxAttemptsDraft(nextValue)

                    const value = parseInt(nextValue, 10)
                    if (
                      !Number.isNaN(value) &&
                      value >= RECONNECT_MAX_ATTEMPTS_MIN &&
                      value <= RECONNECT_MAX_ATTEMPTS_MAX
                    ) {
                      void handleReconnectMaxAttemptsChange(value)
                    }
                  }}
                  onBlur={commitReconnectMaxAttempts}
                  onKeyDown={(event) => {
                    if (event.key === "Enter" && !isImeKeyEvent(event.nativeEvent)) {
                      event.currentTarget.blur()
                    }
                  }}
                  className="h-8"
                />
              </div>
              <div className="text-muted-foreground pb-2 text-xs">
                {t("settings.reconnectMaxAttemptsRange", {
                  defaultValue: `${RECONNECT_MAX_ATTEMPTS_MIN}-${RECONNECT_MAX_ATTEMPTS_MAX}`,
                })}
              </div>
            </div>
          </SettingsRow>
        </SettingsSection>

        <PredictiveEchoSection />

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
          icon={<KeyRound size={16} />}
          title={t("settings.sudoAutofill")}
          description={t("settings.sudoAutofillDesc")}
        >
          <SettingsRow
            icon={<Asterisk size={16} />}
            title={t("settings.passwordFeedback")}
            description={t("settings.passwordFeedbackDesc")}
            action={
              <Switch checked={passwordFeedback} onCheckedChange={handlePasswordFeedbackChange} />
            }
          />
          <SudoPromptPatternsSetting
            onChange={handleSudoPromptPatternsChange}
            patterns={sudoPromptPatterns}
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
                    if (event.key === "Enter" && !isImeKeyEvent(event.nativeEvent)) {
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
