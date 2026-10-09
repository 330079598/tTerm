import React from "react"
import { useTranslation } from "react-i18next"

import { agentDisplayName } from "@/lib/agentStatus"
import { useTabAgentStatus, useTabAttention } from "@/lib/tabAttention"

/**
 * The mark on a tab: an AI agent at work or waiting in it, otherwise a dot
 * for news (new output, a bell, a notification).
 */
export const TabAttentionDot: React.FC<{ tabId: string }> = ({ tabId }) => {
  const { t } = useTranslation()
  const level = useTabAttention(tabId)
  const agent = useTabAgentStatus(tabId)
  if (agent) {
    const label = t(`notifications.agentActivity.${agent.activity}`, {
      agent: agentDisplayName(agent.agent),
    })
    return (
      <span
        className="tab-agent"
        data-activity={agent.activity}
        role="img"
        aria-label={label}
        title={label}
      />
    )
  }
  if (!level) return null
  const label = t(`notifications.attention.${level}`)
  return <span className="tab-attention" data-level={level} role="img" aria-label={label} />
}
