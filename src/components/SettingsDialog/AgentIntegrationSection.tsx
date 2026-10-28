import React, { useCallback, useEffect, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { open as openDirectoryDialog } from "@tauri-apps/plugin-dialog"
import { Bot, CircleCheck, Hourglass } from "lucide-react"
import { useTranslation } from "react-i18next"

import { AGENT_LINK_CLASS, AgentIntegrationRow } from "@/components/AgentIntegrationRow"
import { SettingsRow, SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import { Switch } from "@/components/ui/switch"
import { useConfig } from "@/contexts/ConfigContext"
import { toast } from "@/hooks/use-toast"
import { useSettingsSave } from "@/hooks/useSettingsSave"
import { toErrorMessage } from "@/lib/utils"
import type { AgentIntegrationStatus } from "@/types/agentIntegration"

/** The hooks that let AI agents report to tTerm, and what their reports announce. */
export const AgentIntegrationSection: React.FC = () => {
  const { t } = useTranslation()
  const { config } = useConfig()
  const { saveSettings } = useSettingsSave()
  const [statuses, setStatuses] = useState<AgentIntegrationStatus[]>([])
  const [busyAgent, setBusyAgent] = useState<string | null>(null)

  const refresh = useCallback(() => {
    invoke<AgentIntegrationStatus[]>("agent_integration_status")
      .then(setStatuses)
      .catch((error: unknown) => console.error("Failed to read agent integrations:", error))
  }, [])

  useEffect(() => {
    refresh()
    // The settings file may have been edited meanwhile.
    window.addEventListener("focus", refresh)
    return () => window.removeEventListener("focus", refresh)
  }, [refresh])

  const change = async (status: AgentIntegrationStatus, install: boolean) => {
    setBusyAgent(status.agent)
    try {
      const next = await invoke<AgentIntegrationStatus>(
        install ? "install_agent_integration" : "uninstall_agent_integration",
        { agent: status.agent }
      )
      setStatuses((current) => current.map((item) => (item.agent === next.agent ? next : item)))
    } catch (error) {
      toast({
        title: install
          ? t("notifications.agents.installFailed")
          : t("notifications.agents.uninstallFailed"),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    } finally {
      setBusyAgent(null)
    }
  }

  const chooseConfigDir = async (status: AgentIntegrationStatus) => {
    const selected = await openDirectoryDialog({
      directory: true,
      multiple: false,
      defaultPath: status.configDir || undefined,
    }).catch(() => null)
    if (typeof selected !== "string") return
    const dirs = { ...config.agent_config_dirs, [status.agent]: selected }
    if (await saveSettings({ agent_config_dirs: dirs })) refresh()
  }

  const resetConfigDir = async (status: AgentIntegrationStatus) => {
    const dirs = Object.fromEntries(
      Object.entries(config.agent_config_dirs).filter(([agent]) => agent !== status.agent)
    )
    if (await saveSettings({ agent_config_dirs: dirs })) refresh()
  }

  return (
    <SettingsSection
      icon={<Bot size={16} />}
      title={t("notifications.agents.title")}
      description={t("notifications.agents.description")}
    >
      {statuses.map((status) => {
        const busy = busyAgent === status.agent
        return (
          <AgentIntegrationRow
            key={status.agent}
            status={status}
            place="local"
            busy={busy}
            onChange={(install) => void change(status, install)}
            links={
              <>
                <button
                  type="button"
                  className={AGENT_LINK_CLASS}
                  disabled={busy}
                  onClick={() => void chooseConfigDir(status)}
                >
                  {t("notifications.agents.chooseConfigDir")}
                </button>
                {status.configSource === "setting" && (
                  <button
                    type="button"
                    className={AGENT_LINK_CLASS}
                    disabled={busy}
                    onClick={() => void resetConfigDir(status)}
                  >
                    {t("notifications.agents.resetConfigDir")}
                  </button>
                )}
              </>
            }
          />
        )
      })}
      <SettingsRow
        icon={<Hourglass size={16} />}
        title={t("notifications.agents.notifyWaiting")}
        description={t("notifications.agents.notifyWaitingDesc")}
        action={
          <Switch
            checked={config.notify_agent_waiting}
            onCheckedChange={(checked) => void saveSettings({ notify_agent_waiting: checked })}
            aria-label={t("notifications.agents.notifyWaiting")}
          />
        }
      />
      <SettingsRow
        icon={<CircleCheck size={16} />}
        title={t("notifications.agents.notifyDone")}
        description={t("notifications.agents.notifyDoneDesc")}
        action={
          <Switch
            checked={config.notify_agent_done}
            onCheckedChange={(checked) => void saveSettings({ notify_agent_done: checked })}
            aria-label={t("notifications.agents.notifyDone")}
          />
        }
      />
    </SettingsSection>
  )
}
