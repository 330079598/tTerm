import React from "react"
import { useTranslation } from "react-i18next"

import { useTabLogRecording } from "@/lib/terminalLogRecording"

interface TabLogIndicatorProps {
  tabId: string
  /** A tab of the bar with split panes: the panes it shows the mark for. */
  paneIds?: readonly string[]
}

/** The mark on a tab whose session is being written to a log. */
export const TabLogIndicator: React.FC<TabLogIndicatorProps> = ({ tabId, paneIds }) => {
  const { t } = useTranslation()
  const recording = useTabLogRecording(paneIds ?? [tabId])
  if (!recording) return null
  const label = t("terminalLogging.tabRecording")
  return <span className="tab-log-recording" role="img" aria-label={label} title={label} />
}
