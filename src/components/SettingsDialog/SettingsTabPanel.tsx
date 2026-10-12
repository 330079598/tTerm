import React from "react"

import { TabsContent } from "@/components/ui/tabs"
import { cn } from "@/lib/utils"

interface SettingsTabPanelProps {
  value: string
  /** Scrolls the page itself instead of leaving it to the tab. */
  scroll?: boolean
  children: React.ReactNode
}

/** One settings page, labelled by its tab in the sidebar. */
export const SettingsTabPanel: React.FC<SettingsTabPanelProps> = ({ value, scroll, children }) => (
  <TabsContent
    value={value}
    id={`settings-panel-${value}`}
    role="tabpanel"
    aria-labelledby={`settings-tab-${value}`}
    tabIndex={0}
    className={cn(
      "m-0 min-w-0 flex-1",
      scroll ? "overflow-y-auto p-6" : "overflow-hidden p-4 sm:p-6"
    )}
  >
    {children}
  </TabsContent>
)
