import React from "react"

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { useConfig } from "@/contexts/ConfigContext"
import { tabTooltip } from "@/lib/localTabTitle"
import type { Tab } from "@/types/tab"

interface TabTooltipProps {
  tab: Tab
  /** The tab's header element, which receives the hover. */
  children: React.ReactElement
}

/** The hover tooltip on a tab's header, shown only when the setting turns it on. */
export function TabTooltip({ tab, children }: TabTooltipProps) {
  const { config } = useConfig()

  if (!config.tab_hover_tooltip) {
    return children
  }

  return (
    <Tooltip>
      <TooltipTrigger asChild>{children}</TooltipTrigger>
      <TooltipContent>{tabTooltip(tab)}</TooltipContent>
    </Tooltip>
  )
}
