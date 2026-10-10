import React from "react"
import { useTranslation } from "react-i18next"

import { useTabLogRecording } from "@/lib/terminalLogRecording"

/** The mark on a tab whose session is being written to a log. */
export const TabLogIndicator: React.FC<{ tabId: string }> = ({ tabId }) => {
  const { t } = useTranslation()
  const recording = useTabLogRecording(tabId)
  if (!recording) return null
  const label = t("terminalLogging.tabRecording")
  return <span className="tab-log-recording" role="img" aria-label={label} title={label} />
}
