import React, { useState } from "react"
import type { TFunction } from "i18next"
import { useTranslation } from "react-i18next"

import { AgentLogo } from "@/components/AgentLogo"
import { SettingsRow } from "@/components/SettingsDialog/SettingsLayout"
import { Button } from "@/components/ui/button"
import { AGENT_LOGO_COLORS, type AgentLogoId } from "@/lib/tabIcons"
import type { AgentIntegrationStatus } from "@/types/agentIntegration"

const AGENT_NAMES: Record<AgentIntegrationStatus["agent"], string> = {
  claudeCode: "Claude Code",
  codex: "Codex",
  openCode: "OpenCode",
  pi: "pi",
}

/** Each agent's logo, by the name its reports give. */
const AGENT_LOGOS: Record<AgentIntegrationStatus["agent"], AgentLogoId> = {
  claudeCode: "claude",
  codex: "codex",
  openCode: "opencode",
  pi: "pi",
}

/** What the user has to know about an agent's integration. */
const AGENT_NOTES: Partial<Record<AgentIntegrationStatus["agent"], string>> = {
  codex: "notifications.agents.notes.codex",
  openCode: "notifications.agents.notes.openCode",
}

export const AGENT_LINK_CLASS =
  "hover:text-foreground underline-offset-4 hover:underline disabled:opacity-50"

/** Where the agent runs: on this computer, or on the tab's SSH host. */
export type AgentPlace = "local" | "remote"

function describe(status: AgentIntegrationStatus, place: AgentPlace, t: TFunction) {
  const options = { agent: AGENT_NAMES[status.agent], path: status.configPath }
  if (status.error) return t("notifications.agents.readFailed", { error: status.error })
  if (status.state === "installed") return t("notifications.agents.installedDesc", options)
  if (status.state === "outdated") return t("notifications.agents.outdatedDesc", options)
  if (!status.detected) {
    return place === "remote"
      ? t("remoteAgents.notDetectedDesc", options)
      : t("notifications.agents.notDetectedDesc", options)
  }
  return status.kind === "plugin"
    ? t("notifications.agents.notInstalledPluginDesc", options)
    : t("notifications.agents.notInstalledDesc", options)
}

function changesDescription(status: AgentIntegrationStatus, place: AgentPlace, t: TFunction) {
  const options = { path: status.configPath }
  if (place === "remote") {
    return status.kind === "plugin"
      ? t("remoteAgents.changesPluginDesc", options)
      : t("remoteAgents.changesDesc", options)
  }
  return status.kind === "plugin"
    ? t("notifications.agents.changesPluginDesc", options)
    : t("notifications.agents.changesDesc", options)
}

interface AgentIntegrationRowProps {
  status: AgentIntegrationStatus
  place: AgentPlace
  busy: boolean
  onChange: (install: boolean) => void
  /** Links shown before "Show what is added", such as choosing the folder. */
  links?: React.ReactNode
}

/**
 * One agent's integration: its state, installing, updating or removing it,
 * and what tTerm adds, on this computer or on an SSH host.
 */
export const AgentIntegrationRow: React.FC<AgentIntegrationRowProps> = ({
  status,
  place,
  busy,
  onChange,
  links,
}) => {
  const { t } = useTranslation()
  const [expanded, setExpanded] = useState(false)
  const note = AGENT_NOTES[status.agent]
  const logo = AGENT_LOGOS[status.agent]
  const sourceKey =
    place === "remote" && status.configSource === "environment"
      ? "remoteAgents.environmentSource"
      : `notifications.agents.configSources.${status.configSource}`

  return (
    <SettingsRow
      icon={<AgentLogo agent={logo} size={16} color={AGENT_LOGO_COLORS[logo]} />}
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
      description={describe(status, place, t)}
      action={
        <div className="flex gap-2">
          {status.state !== "installed" && (
            <Button
              type="button"
              disabled={busy || status.error !== null}
              onClick={() => onChange(true)}
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
              onClick={() => onChange(false)}
            >
              {t("notifications.agents.uninstall")}
            </Button>
          )}
        </div>
      }
    >
      {note && status.state !== "notInstalled" && (
        <div className="text-muted-foreground mb-1 text-xs">{t(note)}</div>
      )}
      <div className="text-muted-foreground flex flex-wrap items-center gap-x-3 gap-y-1 text-xs">
        <span>{t(sourceKey)}</span>
        {links}
        <button
          type="button"
          className={AGENT_LINK_CLASS}
          aria-expanded={expanded}
          onClick={() => setExpanded(!expanded)}
        >
          {expanded ? t("notifications.agents.hideChanges") : t("notifications.agents.showChanges")}
        </button>
      </div>
      {expanded && (
        <div className="mt-2 space-y-1.5">
          <div className="text-muted-foreground text-xs">
            {changesDescription(status, place, t)}
          </div>
          <pre className="bg-muted max-h-64 overflow-auto rounded-md p-3 font-mono text-xs select-text">
            {status.preview}
          </pre>
        </div>
      )}
    </SettingsRow>
  )
}
