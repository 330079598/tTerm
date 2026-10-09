import React from "react"
import { useTranslation } from "react-i18next"

import { useTabAttention } from "@/lib/tabAttention"

/** The dot on a tab that has news: new output, a bell, a notification. */
export const TabAttentionDot: React.FC<{ tabId: string }> = ({ tabId }) => {
  const { t } = useTranslation()
  const level = useTabAttention(tabId)
  if (!level) return null
  const label = t(`notifications.attention.${level}`)
  return <span className="tab-attention" data-level={level} role="img" aria-label={label} />
}
