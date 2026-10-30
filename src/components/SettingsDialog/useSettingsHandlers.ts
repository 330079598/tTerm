import type { TFunction } from "i18next"
import { invoke } from "@tauri-apps/api/core"

import type { AppConfig, BackgroundThrottling, CloseBehavior } from "@/lib/appConfig"
import type { useToast } from "@/hooks/use-toast"
import type { useConfirmDialog } from "@/components/ui/app-dialog"
import type { useSettingsSave } from "@/hooks/useSettingsSave"
import { toErrorMessage } from "@/lib/utils"
import type { UpdateChannel, UpdateCheckFrequency } from "@/lib/updater"

interface UseSettingsHandlersOptions {
  confirm: ReturnType<typeof useConfirmDialog>["confirm"]
  t: TFunction
  saveSettings: ReturnType<typeof useSettingsSave>["saveSettings"]
  toast: ReturnType<typeof useToast>["toast"]
}

/**
 * Saving the settings of the general, connection, logging and update tabs,
 * asking first where a change needs it.
 */
export function useSettingsHandlers({
  confirm,
  t,
  saveSettings,
  toast,
}: UseSettingsHandlersOptions) {
  const handleClearSession = async () => {
    const confirmed = await confirm({
      title: t("settings.clearSession"),
      description: t("settings.clearSessionConfirm"),
      confirmText: t("settings.clearSession"),
      cancelText: t("common.cancel"),
      variant: "destructive",
    })

    if (confirmed) {
      window.location.reload()
    }
  }

  const handleRestoreAllSessionConnectionsChange = async (checked: boolean) => {
    await saveSettings({ startup_session_restore_mode: checked ? "all" : "active" })
  }

  const handleRestoreTerminalContentChange = async (checked: boolean) => {
    await saveSettings({ restore_terminal_content: checked })
  }

  const handleZmodemAutoDetectEnabledChange = async (checked: boolean) => {
    await saveSettings({ zmodem_auto_detect_enabled: checked })
  }

  const handleZmodemDownloadDirectoryChange = async (directory: string) => {
    await saveSettings({ zmodem_download_directory: directory })
  }

  const handleBackgroundThrottlingChange = async (policy: BackgroundThrottling) => {
    await saveSettings({ background_throttling: policy })
  }

  const handleCloseBehaviorChange = async (behavior: CloseBehavior) => {
    await saveSettings({ close_behavior: behavior })
  }

  const handleEnableDevtoolsChange = async () => {
    try {
      await invoke("toggle_devtools", { enable: true })
    } catch (error) {
      console.error("Failed to open devtools:", error)
      toast({
        title: t("settings.saveFailed", { defaultValue: "Failed to save settings" }),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    }
  }

  const handleReconnectEnabledChange = async (checked: boolean) => {
    await saveSettings({ reconnect_enabled: checked })
  }

  const handleReconnectMaxAttemptsChange = async (attempts: number) => {
    await saveSettings({ reconnect_max_attempts: attempts })
  }

  const handleShowJumpHostConnectionInfoChange = async (checked: boolean) => {
    await saveSettings({ show_jump_host_connection_info: checked })
  }

  const handleSudoPromptPatternsChange = async (patterns: string[]) => {
    await saveSettings({ sudo_prompt_patterns: patterns })
  }

  const handlePasswordFeedbackChange = async (checked: boolean) => {
    await saveSettings({ password_feedback: checked })
  }

  const handleMonitorRefreshIntervalChange = async (seconds: number) => {
    await saveSettings({ monitor_refresh_interval_secs: seconds })
  }

  const handleMonitorVisibleMetricsChange = async (
    metrics: AppConfig["monitor_visible_metrics"]
  ) => {
    await saveSettings({ monitor_visible_metrics: metrics })
  }

  const handleTerminalLogEnabledChange = async (enabled: boolean) => {
    if (enabled) {
      const confirmed = await confirm({
        title: t("terminalLogging.confirmTitle"),
        description: t("terminalLogging.confirmDescription"),
        confirmText: t("terminalLogging.enable"),
        cancelText: t("common.cancel"),
      })
      if (!confirmed) return
    }
    await saveSettings({ terminal_log_enabled: enabled })
  }

  const handleTerminalLogRecordInputChange = async (recordInput: boolean) => {
    if (recordInput) {
      const confirmed = await confirm({
        title: t("terminalLogging.recordInputConfirmTitle"),
        description: t("terminalLogging.recordInputConfirmDescription"),
        confirmText: t("terminalLogging.recordInputConfirm"),
        cancelText: t("common.cancel"),
      })
      if (!confirmed) return
    }
    await saveSettings({ terminal_log_record_input: recordInput })
  }

  const handleUpdateChannelChange = async (channel: UpdateChannel) => {
    await saveSettings({ update_channel: channel })
  }

  const handleAutoDownloadUpdatesChange = async (checked: boolean) => {
    await saveSettings({ auto_download_updates: checked })
  }

  const handleUpdateCheckFrequencyChange = async (frequency: UpdateCheckFrequency) => {
    await saveSettings({ update_check_frequency: frequency })
  }

  return {
    handleClearSession,
    handleRestoreAllSessionConnectionsChange,
    handleRestoreTerminalContentChange,
    handleZmodemAutoDetectEnabledChange,
    handleZmodemDownloadDirectoryChange,
    handleBackgroundThrottlingChange,
    handleCloseBehaviorChange,
    handleEnableDevtoolsChange,
    handleReconnectEnabledChange,
    handleReconnectMaxAttemptsChange,
    handleShowJumpHostConnectionInfoChange,
    handleSudoPromptPatternsChange,
    handlePasswordFeedbackChange,
    handleMonitorRefreshIntervalChange,
    handleMonitorVisibleMetricsChange,
    handleTerminalLogEnabledChange,
    handleTerminalLogRecordInputChange,
    handleUpdateChannelChange,
    handleAutoDownloadUpdatesChange,
    handleUpdateCheckFrequencyChange,
  }
}
