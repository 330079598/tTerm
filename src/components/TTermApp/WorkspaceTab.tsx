import React from "react"
import type { IDockviewPanelHeaderProps } from "dockview-react"
import { X } from "lucide-react"
import { useTranslation } from "react-i18next"

import { TabAttentionDot } from "@/components/TabAttentionDot"
import { TabIcon } from "@/components/TabIcon"
import { TabLogIndicator } from "@/components/TabLogIndicator"
import { TabTitle } from "@/components/TabTitle"
import {
  useDockPanelState,
  useWorkspace,
  type WorkspacePanelParams,
} from "@/components/TTermApp/workspaceContext"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { tabTooltip } from "@/lib/localTabTitle"
import { getWorkspaceTabMenuActions } from "@/lib/tabMenuActions"

/** A tab's header in a workspace group, with its context menu and tooltip. */
export const WorkspaceTab = React.memo(function WorkspaceTab({
  api,
  params,
}: IDockviewPanelHeaderProps<WorkspacePanelParams>) {
  const { t } = useTranslation()
  const { onTabClose, onTabContextMenu, tabs } = useWorkspace()
  const { isVisible } = useDockPanelState(api)
  const tab = tabs.find((candidate) => candidate.id === params.tabId)

  if (!tab) {
    return null
  }

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <div
          className={`workspace-tab ${isVisible ? "active" : ""} ${tab.isModified ? "modified" : ""}`}
          role="tab"
          aria-selected={isVisible}
          data-allow-context-menu
          onClick={() => api.setActive()}
          onContextMenu={(event) =>
            onTabContextMenu(
              event,
              tab,
              getWorkspaceTabMenuActions(
                tab,
                api.group.panels.map((panel) => panel.id),
                t
              )
            )
          }
        >
          <TabIcon tab={tab} />
          <TabAttentionDot tabId={tab.id} />
          <TabLogIndicator tabId={tab.id} />
          <TabTitle tab={tab} className="workspace-tab-title" />
          <button
            type="button"
            className="workspace-tab-close"
            aria-label={t("contextMenu.closeTab", { defaultValue: "Close tab" })}
            onClick={(event) => {
              event.stopPropagation()
              onTabClose(tab.id)
            }}
          >
            <X size={12} aria-hidden="true" />
          </button>
        </div>
      </TooltipTrigger>
      <TooltipContent>{tabTooltip(tab)}</TooltipContent>
    </Tooltip>
  )
})
