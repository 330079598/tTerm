import type React from "react"
import { useTranslation } from "react-i18next"

import { useConfig } from "@/contexts/ConfigContext"
import { agentDisplayName, type AgentStatus } from "@/lib/agentStatus"
import { tabAgentColorVariables } from "@/lib/tabAgentIndicator"
import { useTabAgentStatus, useTabAttention } from "@/lib/tabAttention"

interface TabAgentBadgeProps {
  activity: AgentStatus["activity"]
  /** A style from `TAB_AGENT_WORKING_STYLES` or `TAB_AGENT_WAITING_STYLES`, to match `activity`. */
  style: string
  /** A `#rrggbb` color, or "" for the theme's. */
  color: string
  label: string
}

/** The badge for an AI agent at work or waiting. */
export function TabAgentBadge({ activity, style, color, label }: TabAgentBadgeProps) {
  return (
    <span
      className="tab-agent"
      data-activity={activity}
      data-style={style}
      style={tabAgentColorVariables(color) as React.CSSProperties | undefined}
      role="img"
      aria-label={label}
      title={label}
    >
      {/* Each dot bounces on its own, so each needs an element. */}
      {activity === "processing" && style === "dots" && (
        <>
          <i />
          <i />
          <i />
        </>
      )}
    </span>
  )
}

/**
 * The mark on a tab: an AI agent at work or waiting in it, otherwise a dot
 * for news (new output, a bell, a notification).
 */
export const TabAttentionDot: React.FC<{ tabId: string }> = ({ tabId }) => {
  const { t } = useTranslation()
  const { config } = useConfig()
  const level = useTabAttention(tabId)
  const agent = useTabAgentStatus(tabId)
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
