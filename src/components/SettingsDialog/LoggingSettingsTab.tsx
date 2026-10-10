import React, { useCallback, useEffect, useMemo, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { open as openDirectoryDialog } from "@tauri-apps/plugin-dialog"
import {
  Archive,
  CalendarClock,
  CheckCircle2,
  CircleAlert,
  CircleDot,
  Eraser,
  FileClock,
  FileText,
  FolderOpen,
  HardDrive,
  Keyboard,
  ScrollText,
} from "lucide-react"
import { useTranslation } from "react-i18next"

import { SettingsRow, SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Switch } from "@/components/ui/switch"
import {
  TERMINAL_LOG_MAX_RETENTION_DAYS,
  TERMINAL_LOG_MAX_TOTAL_MB,
  type TerminalLogFormat,
  useConfig,
} from "@/contexts/ConfigContext"
import { useSettingsSave } from "@/hooks/useSettingsSave"
import { requestAppPage } from "@/lib/appNavigation"
import { formatLogSize, type TerminalLogStatus } from "@/lib/terminalLogRecording"
import { cn, toErrorMessage } from "@/lib/utils"

interface LoggingSettingsTabProps {
  handleEnabledChange: (enabled: boolean) => Promise<void>
  handleRecordInputChange: (recordInput: boolean) => Promise<void>
}

/** A whole number in `0..=max` typed into a field, or null. */
function parseLimit(value: string, max: number): number | null {
  const parsed = Number(value)
  return Number.isInteger(parsed) && parsed >= 0 && parsed <= max ? parsed : null
}

function renderExample(template: string) {
  return [
    ["{profile}", "production"],
    ["{host}", "10.0.0.8"],
    ["{port}", "22"],
    ["{username}", "root"],
    ["{type}", "ssh"],
    ["{date}", "20260729"],
    ["{time}", "153012"],
    ["{yyyyMMdd-HHmmss}", "20260729-153012"],
    ["{sessionId}", "a83f"],
  ].reduce((value, [token, replacement]) => value.split(token).join(replacement), template)
}

export const LoggingSettingsTab: React.FC<LoggingSettingsTabProps> = ({
  handleEnabledChange,
  handleRecordInputChange,
}) => {
  const { t } = useTranslation()
  const { config } = useConfig()
  const { saveSettings } = useSettingsSave()
  const [status, setStatus] = useState<TerminalLogStatus | null>(null)
  const [statusError, setStatusError] = useState<string | null>(null)
  const [nameTemplate, setNameTemplate] = useState(config.terminal_log_name_template)
  const [nameError, setNameError] = useState<string | null>(null)
  const [maxFileSize, setMaxFileSize] = useState(String(config.terminal_log_max_file_size_mb))
  const [sizeError, setSizeError] = useState<string | null>(null)
  const [retentionDays, setRetentionDays] = useState(String(config.terminal_log_retention_days))
  const [retentionError, setRetentionError] = useState<string | null>(null)
  const [maxTotal, setMaxTotal] = useState(String(config.terminal_log_max_total_mb))
  const [maxTotalError, setMaxTotalError] = useState<string | null>(null)

  const refreshStatus = useCallback(async () => {
    try {
      setStatus(await invoke<TerminalLogStatus>("get_terminal_log_status"))
      setStatusError(null)
    } catch (error) {
      setStatusError(toErrorMessage(error))
    }
  }, [])

  useEffect(() => {
    let disposed = false
    const cleanups: Array<() => void> = []
    void invoke<TerminalLogStatus>("get_terminal_log_status")
      .then((nextStatus) => {
        if (!disposed) setStatus(nextStatus)
      })
      .catch((error) => {
        if (!disposed) setStatusError(toErrorMessage(error))
      })
    void Promise.all([
      listen<TerminalLogStatus>("terminal-log-status", (event) => {
        if (!disposed) setStatus(event.payload)
      }),
      listen<{ message?: string }>("terminal-log-error", (event) => {
        if (!disposed) {
          setStatusError(event.payload.message ?? t("terminalLogging.writeFailed"))
          void refreshStatus()
        }
      }),
    ]).then((unlisteners) => {
      if (disposed) {
        unlisteners.forEach((unlisten) => unlisten())
      } else {
        cleanups.push(...unlisteners)
      }
    })
    const statusInterval = window.setInterval(() => void refreshStatus(), 5_000)
    return () => {
      disposed = true
      window.clearInterval(statusInterval)
      cleanups.forEach((cleanup) => cleanup())
    }
  }, [refreshStatus, t])

  const resolvedDirectory = status?.directory || config.terminal_log_directory
  const exampleName = useMemo(() => renderExample(nameTemplate), [nameTemplate])

  const saveFormat = async (format: TerminalLogFormat) => {
    if (await saveSettings({ terminal_log_format: format })) void refreshStatus()
  }

  const chooseDirectory = async () => {
    const selected = await openDirectoryDialog({
      directory: true,
      multiple: false,
      defaultPath: resolvedDirectory || undefined,
    }).catch(() => null)
    if (typeof selected !== "string") return
    if (await saveSettings({ terminal_log_directory: selected })) void refreshStatus()
  }

  const saveNameTemplate = async () => {
    const trimmed = nameTemplate.trim()
    if (!trimmed) {
      setNameError(t("terminalLogging.nameRequired"))
      return
    }
    if (trimmed.length > 180) {
      setNameError(t("terminalLogging.nameTooLong"))
      return
    }
    setNameError(null)
    if (await saveSettings({ terminal_log_name_template: trimmed })) {
      setNameTemplate(trimmed)
      void refreshStatus()
    }
  }

  const saveMaxFileSize = async () => {
    const value = Number(maxFileSize)
    if (!Number.isInteger(value) || value < 1 || value > 1024) {
      setSizeError(t("terminalLogging.sizeRange"))
      return
    }
    setSizeError(null)
    if (await saveSettings({ terminal_log_max_file_size_mb: value })) {
      setMaxFileSize(String(value))
      void refreshStatus()
    }
  }

  const saveRetentionDays = async () => {
    const value = parseLimit(retentionDays, TERMINAL_LOG_MAX_RETENTION_DAYS)
    if (value === null) {
      setRetentionError(
        t("terminalLogging.retentionDaysRange", { max: TERMINAL_LOG_MAX_RETENTION_DAYS })
      )
      return
    }
    setRetentionError(null)
    if (value === config.terminal_log_retention_days) return
    if (await saveSettings({ terminal_log_retention_days: value })) {
      setRetentionDays(String(value))
      void refreshStatus()
    }
  }

  const saveMaxTotal = async () => {
    const value = parseLimit(maxTotal, TERMINAL_LOG_MAX_TOTAL_MB)
    if (value === null) {
      setMaxTotalError(
        t("terminalLogging.maxTotalSizeRange", {
          max: TERMINAL_LOG_MAX_TOTAL_MB.toLocaleString(),
        })
      )
      return
    }
    setMaxTotalError(null)
    if (value === config.terminal_log_max_total_mb) return
    if (await saveSettings({ terminal_log_max_total_mb: value })) {
      setMaxTotal(String(value))
      void refreshStatus()
    }
  }

  const retryLogging = async () => {
    try {
      setStatus(await invoke<TerminalLogStatus>("retry_terminal_logging"))
      setStatusError(null)
    } catch (error) {
      setStatusError(toErrorMessage(error))
    }
  }

  return (
    <ScrollArea className="h-full pr-4">
      <div className="space-y-6">
        <SettingsSection
          icon={<FileClock size={16} />}
          title={t("terminalLogging.title")}
          description={t("terminalLogging.description")}
        >
          <SettingsRow
            icon={<CircleDot size={16} />}
            title={t("terminalLogging.enabled")}
            description={t("terminalLogging.enabledDesc")}
            action={
              <Switch
                checked={config.terminal_log_enabled}
                onCheckedChange={(checked) => void handleEnabledChange(checked)}
                aria-label={t("terminalLogging.enabled")}
              />
            }
          />
          <SettingsRow
            icon={<Keyboard size={16} />}
            title={t("terminalLogging.recordInput")}
            description={t("terminalLogging.recordInputDesc")}
            action={
              <Switch
                checked={config.terminal_log_record_input}
                onCheckedChange={(checked) => void handleRecordInputChange(checked)}
                aria-label={t("terminalLogging.recordInput")}
              />
            }
          />
          <SettingsRow
            icon={<ScrollText size={16} />}
            title={t("terminalLogging.browse")}
            description={t("terminalLogging.browseDesc")}
            action={
              <Button
                type="button"
                variant="outline"
                onClick={() => requestAppPage("terminal-logs")}
              >
                {t("terminalLogging.browseButton")}
              </Button>
            }
          />
        </SettingsSection>

        <Alert className="border-amber-500/40 bg-amber-500/5">
          <CircleAlert className="absolute top-3.5 left-4 size-4 text-amber-500" />
          <div className="pl-6">
            <AlertTitle>{t("terminalLogging.securityTitle")}</AlertTitle>
            <AlertDescription>
              {config.terminal_log_record_input
                ? t("terminalLogging.securityDescriptionInput")
                : t("terminalLogging.securityDescription")}
            </AlertDescription>
          </div>
        </Alert>

        <SettingsSection icon={<HardDrive size={16} />} title={t("terminalLogging.storage")}>
          <SettingsRow
            icon={<FolderOpen size={16} />}
            title={t("terminalLogging.directory")}
            description={t("terminalLogging.directoryDesc")}
          >
            <div className="flex min-w-0 gap-2">
              <Input
                value={resolvedDirectory}
                readOnly
                aria-label={t("terminalLogging.directory")}
                className="min-w-0 flex-1 font-mono text-xs"
              />
              <Button type="button" variant="outline" onClick={() => void chooseDirectory()}>
                <FolderOpen />
                {t("terminalLogging.choose")}
              </Button>
              <Button
                type="button"
                variant="outline"
                disabled={!resolvedDirectory}
                onClick={() =>
                  void invoke("open_terminal_log_directory").catch((error) =>
                    setStatusError(toErrorMessage(error))
                  )
                }
              >
                <FolderOpen />
                {t("terminalLogging.openDirectory")}
              </Button>
            </div>
          </SettingsRow>

          <SettingsRow
            icon={<FileText size={16} />}
            title={t("terminalLogging.format")}
            description={t("terminalLogging.formatDesc")}
          >
            <div
              className="grid grid-cols-3 gap-2"
              role="group"
              aria-label={t("terminalLogging.format")}
            >
              {(["raw", "plain", "both"] as const).map((format) => (
                <Button
                  key={format}
                  type="button"
                  variant={config.terminal_log_format === format ? "default" : "outline"}
                  aria-pressed={config.terminal_log_format === format}
                  onClick={() => void saveFormat(format)}
                >
                  {t(`terminalLogging.formats.${format}`)}
                </Button>
              ))}
            </div>
          </SettingsRow>

          <SettingsRow
            icon={<FileText size={16} />}
            title={t("terminalLogging.nameTemplate")}
            description={t("terminalLogging.nameTemplateDesc")}
          >
            <Label htmlFor="terminal-log-name" className="sr-only">
              {t("terminalLogging.nameTemplate")}
            </Label>
            <Input
              id="terminal-log-name"
              value={nameTemplate}
              aria-invalid={Boolean(nameError)}
              aria-describedby="terminal-log-name-help"
              onChange={(event) => {
                setNameTemplate(event.target.value)
                setNameError(null)
              }}
              onBlur={() => void saveNameTemplate()}
              className="font-mono text-xs"
            />
            <p
              id="terminal-log-name-help"
              role={nameError ? "alert" : undefined}
              className={cn(
                "mt-2 text-xs",
                nameError ? "text-destructive" : "text-muted-foreground"
              )}
            >
              {nameError ?? t("terminalLogging.nameExample", { name: exampleName })}
            </p>
          </SettingsRow>

          <SettingsRow
            icon={<HardDrive size={16} />}
            title={t("terminalLogging.maxFileSize")}
            description={t("terminalLogging.maxFileSizeDesc")}
          >
            <div className="flex items-center gap-2">
              <Label htmlFor="terminal-log-size" className="sr-only">
                {t("terminalLogging.maxFileSize")}
              </Label>
              <Input
                id="terminal-log-size"
                type="number"
                min={1}
                max={1024}
                step={1}
                value={maxFileSize}
                aria-invalid={Boolean(sizeError)}
                onChange={(event) => {
                  setMaxFileSize(event.target.value)
                  setSizeError(null)
                }}
                onBlur={() => void saveMaxFileSize()}
                className="w-32"
              />
              <span className="text-muted-foreground text-sm">MiB</span>
            </div>
            {sizeError && (
              <p role="alert" className="text-destructive mt-2 text-xs">
                {sizeError}
              </p>
            )}
          </SettingsRow>

          <SettingsRow
            icon={<Archive size={16} />}
            title={t("terminalLogging.compress")}
            description={t("terminalLogging.compressDesc")}
            action={
              <Switch
                checked={config.terminal_log_compress}
                onCheckedChange={async (checked) => {
                  if (await saveSettings({ terminal_log_compress: checked })) void refreshStatus()
                }}
                aria-label={t("terminalLogging.compress")}
              />
            }
          />
        </SettingsSection>

        <SettingsSection
          icon={<Eraser size={16} />}
          title={t("terminalLogging.cleanup")}
          description={t("terminalLogging.cleanupDesc")}
        >
          <SettingsRow
            icon={<CalendarClock size={16} />}
            title={t("terminalLogging.retentionDays")}
            description={t("terminalLogging.retentionDaysDesc")}
          >
            <div className="flex items-center gap-2">
              <Label htmlFor="terminal-log-retention" className="sr-only">
                {t("terminalLogging.retentionDays")}
              </Label>
              <Input
                id="terminal-log-retention"
                type="number"
                min={0}
                max={TERMINAL_LOG_MAX_RETENTION_DAYS}
                step={1}
                value={retentionDays}
                aria-invalid={Boolean(retentionError)}
                onChange={(event) => {
                  setRetentionDays(event.target.value)
                  setRetentionError(null)
                }}
                onBlur={() => void saveRetentionDays()}
                className="w-32"
              />
              <span className="text-muted-foreground text-sm">{t("terminalLogging.daysUnit")}</span>
            </div>
            {retentionError && (
              <p role="alert" className="text-destructive mt-2 text-xs">
                {retentionError}
              </p>
            )}
          </SettingsRow>

          <SettingsRow
            icon={<HardDrive size={16} />}
            title={t("terminalLogging.maxTotalSize")}
            description={t("terminalLogging.maxTotalSizeDesc")}
          >
            <div className="flex items-center gap-2">
              <Label htmlFor="terminal-log-max-total" className="sr-only">
                {t("terminalLogging.maxTotalSize")}
              </Label>
              <Input
                id="terminal-log-max-total"
                type="number"
                min={0}
                max={TERMINAL_LOG_MAX_TOTAL_MB}
                step={1}
                value={maxTotal}
                aria-invalid={Boolean(maxTotalError)}
                onChange={(event) => {
                  setMaxTotal(event.target.value)
                  setMaxTotalError(null)
                }}
                onBlur={() => void saveMaxTotal()}
                className="w-32"
              />
              <span className="text-muted-foreground text-sm">MiB</span>
            </div>
            {maxTotalError && (
              <p role="alert" className="text-destructive mt-2 text-xs">
                {maxTotalError}
              </p>
            )}
          </SettingsRow>
        </SettingsSection>

        <SettingsSection icon={<CheckCircle2 size={16} />} title={t("terminalLogging.status")}>
          <div className="border-border bg-muted/20 grid grid-cols-2 gap-4 rounded-md border p-4 text-sm sm:grid-cols-3">
            <div>
              <div className="text-muted-foreground text-xs">{t("terminalLogging.state")}</div>
              <div className="mt-1 font-medium">
                {status?.activeSessions
                  ? t("terminalLogging.recording")
                  : t("terminalLogging.stopped")}
              </div>
            </div>
            <div>
              <div className="text-muted-foreground text-xs">
                {t("terminalLogging.activeSessions")}
              </div>
              <div className="mt-1 font-medium tabular-nums">{status?.activeSessions ?? 0}</div>
            </div>
            <div>
              <div className="text-muted-foreground text-xs">{t("terminalLogging.diskUsage")}</div>
              <div className="mt-1 font-medium tabular-nums">
                {formatLogSize(status?.totalSizeBytes ?? 0)}
              </div>
            </div>
          </div>
          {(statusError || status?.lastError) && (
            <div className="mt-2 flex items-start justify-between gap-3">
              <p role="alert" className="text-destructive min-w-0 text-xs">
                {statusError || status?.lastError}
              </p>
              <Button type="button" variant="outline" size="sm" onClick={() => void retryLogging()}>
                {t("terminalLogging.retry")}
              </Button>
            </div>
          )}
        </SettingsSection>
      </div>
    </ScrollArea>
  )
}
