import React, { useCallback, useEffect, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { open as openDirectoryDialog } from "@tauri-apps/plugin-dialog"
import { Bot, CircleCheck, Hourglass, Plug } from "lucide-react"
import { useTranslation } from "react-i18next"

import { SettingsRow, SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import { Button } from "@/components/ui/button"
import { Switch } from "@/components/ui/switch"
import { useConfig } from "@/contexts/ConfigContext"
import { toast } from "@/hooks/use-toast"
import { useSettingsSave } from "@/hooks/useSettingsSave"
import { toErrorMessage } from "@/lib/utils"

/** `AgentIntegrationStatus` (src-tauri/src/agent_integration). */
interface AgentIntegrationStatus {
  agent: "claudeCode" | "codex" | "openCode" | "pi"
  /** Merged into the agent's settings, or a file of tTerm's own. */
  kind: "hooks" | "plugin"
  state: "notInstalled" | "installed" | "outdated"
  detected: boolean
  configDir: string
  configPath: string
  configSource: "setting" | "environment" | "default"
  preview: string
  error: string | null
}

const AGENT_NAMES: Record<AgentIntegrationStatus["agent"], string> = {
  claudeCode: "Claude Code",
  codex: "Codex",
  openCode: "OpenCode",
  pi: "pi",
}

/** What the user has to know about an agent's integration. */
const AGENT_NOTES: Partial<Record<AgentIntegrationStatus["agent"], string>> = {
  codex: "notifications.agents.notes.codex",
  openCode: "notifications.agents.notes.openCode",
}

const LINK_CLASS = "hover:text-foreground underline-offset-4 hover:underline disabled:opacity-50"

/** The hooks that let AI agents report to tTerm, and what their reports announce. */
export const AgentIntegrationSection: React.FC = () => {
  const { t } = useTranslation()
  const { config } = useConfig()
  const { saveSettings } = useSettingsSave()
  const [statuses, setStatuses] = useState<AgentIntegrationStatus[]>([])
  const [busyAgent, setBusyAgent] = useState<string | null>(null)
  const [expandedAgent, setExpandedAgent] = useState<string | null>(null)

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

  const describe = (status: AgentIntegrationStatus) => {
    const options = { agent: AGENT_NAMES[status.agent], path: status.configPath }
    if (status.error) return t("notifications.agents.readFailed", { error: status.error })
    if (status.state === "installed") return t("notifications.agents.installedDesc", options)
    if (status.state === "outdated") return t("notifications.agents.outdatedDesc", options)
    if (!status.detected) return t("notifications.agents.notDetectedDesc", options)
    return status.kind === "plugin"
      ? t("notifications.agents.notInstalledPluginDesc", options)
      : t("notifications.agents.notInstalledDesc", options)
  }

  return (
    <SettingsSection
      icon={<Bot size={16} />}
      title={t("notifications.agents.title")}
      description={t("notifications.agents.description")}
    >
      {statuses.map((status) => {
        const busy = busyAgent === status.agent
        const expanded = expandedAgent === status.agent
        return (
          <SettingsRow
            key={status.agent}
            icon={<Plug size={16} />}
            title={
              <span className="flex items-center gap-2">
                {AGENT_NAMES[status.agent]}
                {status.state !== "notInstalled" && (
                  <span className="text-muted-foreground text-xs font-normal">
                    {t(`notifications.agents.states.${status.state}`)}
                  </span>
                )}
              </span>
            }
            description={describe(status)}
            action={
              <div className="flex gap-2">
                {status.state !== "installed" && (
                  <Button
                    type="button"
                    disabled={busy || status.error !== null}
                    onClick={() => void change(status, true)}
                  >
                    {status.state === "outdated"
                      ? t("notifications.agents.update")
                      : t("notifications.agents.install")}
                  </Button>
                )}
                {status.state !== "notInstalled" && (
                  <Button
                    type="button"
                    variant="outline"
                    disabled={busy || status.error !== null}
                    onClick={() => void change(status, false)}
                  >
                    {t("notifications.agents.uninstall")}
                  </Button>
                )}
              </div>
            }
          >
            {AGENT_NOTES[status.agent] && status.state !== "notInstalled" && (
              <div className="text-muted-foreground mb-1 text-xs">
                {t(AGENT_NOTES[status.agent] as string)}
              </div>
            )}
            <div className="text-muted-foreground flex flex-wrap items-center gap-x-3 gap-y-1 text-xs">
              <span>{t(`notifications.agents.configSources.${status.configSource}`)}</span>
              <button
                type="button"
                className={LINK_CLASS}
                disabled={busy}
                onClick={() => void chooseConfigDir(status)}
              >
                {t("notifications.agents.chooseConfigDir")}
              </button>
              {status.configSource === "setting" && (
                <button
                  type="button"
                  className={LINK_CLASS}
                  disabled={busy}
                  onClick={() => void resetConfigDir(status)}
                >
                  {t("notifications.agents.resetConfigDir")}
                </button>
              )}
              <button
                type="button"
                className={LINK_CLASS}
                aria-expanded={expanded}
                onClick={() => setExpandedAgent(expanded ? null : status.agent)}
              >
                {expanded
                  ? t("notifications.agents.hideChanges")
                  : t("notifications.agents.showChanges")}
              </button>
            </div>
            {expanded && (
              <div className="mt-2 space-y-1.5">
                <div className="text-muted-foreground text-xs">
                  {status.kind === "plugin"
                    ? t("notifications.agents.changesPluginDesc", { path: status.configPath })
                    : t("notifications.agents.changesDesc", { path: status.configPath })}
                </div>
                <pre className="bg-muted max-h-64 overflow-auto rounded-md p-3 font-mono text-xs select-text">
                  {status.preview}
                </pre>
              </div>
            )}
          </SettingsRow>
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
