import type React from "react"
import { KeyRound } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Spinner } from "@/components/ui/spinner"
import { useConfig } from "@/contexts/ConfigContext"
import { useCapsLock } from "@/hooks/useCapsLock"
import { agentDisplayName, type AgentStatus } from "@/lib/agentStatus"
import type { TabAgentWaitingStyle, TabAgentWorkingStyle } from "@/lib/tabAgentIndicator"
import { useTabAgentStatus, useTabAttention, useTabPasswordPrompt } from "@/lib/tabAttention"

interface TabAgentBadgeProps {
  activity: AgentStatus["activity"]
  /** A style from `TAB_AGENT_WORKING_STYLES` or `TAB_AGENT_WAITING_STYLES`, to match `activity`. */
  style: TabAgentWorkingStyle | TabAgentWaitingStyle
  /** A `#rrggbb` color, or "" for the theme's. */
  color: string
  label: string
}

/** The badge for an AI agent at work or waiting. */
export function TabAgentBadge({ activity, style, color, label }: TabAgentBadgeProps) {
  const working = activity === "processing"
  return (
    <span
      className="tab-agent"
      data-activity={activity}
      data-style={working ? undefined : style}
      style={color ? { color } : undefined}
      role="img"
      aria-label={label}
      title={label}
    >
      {working && <Spinner size={8} variant={style as TabAgentWorkingStyle} />}
    </span>
  )
}

interface TabAttentionDotProps {
  tabId: string
  /** A tab of the bar with split panes: the panes whose marks it shows, its active one first. */
  paneIds?: readonly string[]
}

/**
 * The mark on a tab: a key while it waits for a password, which holds up
 * whatever runs there; otherwise an AI agent at work or waiting in it;
 * otherwise a dot for news (new output, a bell, a notification).
 */
export const TabAttentionDot: React.FC<TabAttentionDotProps> = ({ tabId, paneIds }) => {
  const { t } = useTranslation()
  const { config } = useConfig()
  const tabIds = paneIds ?? [tabId]
  const level = useTabAttention(tabIds)
  const agent = useTabAgentStatus(tabIds)
  const waitingForPassword = useTabPasswordPrompt(tabIds)
  const capsLockOn = useCapsLock()
  if (waitingForPassword) {
    const label = t(capsLockOn ? "sudoAutofill.tabWaitingCapsLock" : "sudoAutofill.tabWaiting")
    return (
      <span
        className="tab-password-prompt"
        data-caps-lock={capsLockOn || undefined}
        role="img"
        aria-label={label}
        title={label}
      >
        <KeyRound size={12} aria-hidden="true" />
      </span>
    )
  }
  if (agent) {
    const label = t(`notifications.agentActivity.${agent.activity}`, {
      agent: agentDisplayName(agent.agent),
    })
    const indicator = config.tab_agent_indicator
    const working = agent.activity === "processing"
    return (
      <TabAgentBadge
        activity={agent.activity}
        style={working ? indicator.working : indicator.waiting}
        color={working ? indicator.working_color : indicator.waiting_color}
        label={label}
      />
    )
  }
  if (!level) return null
  const label = t(`notifications.attention.${level}`)
  return <span className="tab-attention" data-level={level} role="img" aria-label={label} />
}
