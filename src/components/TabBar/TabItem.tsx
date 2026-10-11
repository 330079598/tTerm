import React, { useCallback } from "react"
import { useTranslation } from "react-i18next"
import { X } from "lucide-react"

import { TabAttentionDot } from "@/components/TabAttentionDot"
import { TabIcon } from "@/components/TabIcon"
import { TabLogIndicator } from "@/components/TabLogIndicator"
import { TabTitle } from "@/components/TabTitle"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { tabTooltip } from "@/lib/localTabTitle"
import { getTabCloseMenuActions } from "@/lib/tabClosing"
import {
  getTabPinMenuAction,
  getTabRenameMenuAction,
  getTabSplitMenuActions,
} from "@/lib/tabMenuActions"
import { keepFocusOnMouseDown } from "@/lib/terminalFocus"
import { getTabLogMenuActions } from "@/lib/terminalLogRecording"
import { isPageTab, type Tab, type TabContextMenuAction } from "@/types/tab"

interface TabItemProps {
  tab: Tab
  index: number
  getTabContextIds: (tabId: string) => string[]
  isActive: boolean
  isDragging: boolean
  setActiveNode?: (node: HTMLDivElement | null) => void
  onTabClick: (id: string) => void
  onTabClose: (id: string) => void
  onContextMenu: (event: React.MouseEvent, tab: Tab, actions: TabContextMenuAction[]) => void
  onPointerDown: (event: React.PointerEvent<HTMLDivElement>, tabId: string) => void
}

/** One tab in the tab bar, with its context menu. */
export const TabItem = React.memo(function TabItem({
  tab,
  index,
  getTabContextIds,
  isActive,
  isDragging,
  setActiveNode,
  onTabClick,
  onTabClose,
  onContextMenu,
  onPointerDown,
}: TabItemProps) {
  const { t } = useTranslation()

  const setNodeRef = (node: HTMLDivElement | null) => {
    if (isActive) setActiveNode?.(node)
  }

  const handleContextMenu = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault()
      const closeActions = getTabCloseMenuActions(getTabContextIds(tab.id), tab.id, {
        closeTab: t("contextMenu.closeTab"),
        closeOtherTabs: t("contextMenu.closeOtherTabs"),
        closeTabsToLeft: t("contextMenu.closeTabsToLeft"),
        closeTabsToRight: t("contextMenu.closeTabsToRight"),
      })

      if (isPageTab(tab)) {
        onContextMenu(e, tab, [
          { label: t("contextMenu.newTab"), action: "new", icon: "plus" },
          { separator: true, label: "", action: "" },
          ...closeActions,
        ])
        return
      }

      // A tab without a profile (quick connect, or connected without saving) can be saved as one.
      const editConnectionAction: TabContextMenuAction | null =
        tab.type !== "ssh"
          ? null
          : tab.connection?.profileId
            ? { label: t("contextMenu.editConnection"), action: "edit-connection", icon: "edit" }
            : { label: t("contextMenu.saveConnection"), action: "edit-connection", icon: "edit" }

      const logActions = getTabLogMenuActions(tab, t)

      const actions: TabContextMenuAction[] = [
        { label: t("contextMenu.newTab"), action: "new", icon: "plus" },
        { label: t("contextMenu.duplicateTab"), action: "duplicate", icon: "copy" },
        ...getTabSplitMenuActions(tab, t),
        getTabRenameMenuAction(tab, t),
        ...(editConnectionAction ? [editConnectionAction] : []),
        ...getTabPinMenuAction(tab, t),
        ...logActions,
        { separator: true, label: "", action: "" },
        ...closeActions,
      ]
      onContextMenu(e, tab, actions)
    },
    [getTabContextIds, tab, onContextMenu, t]
  )

  const handleKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLDivElement>) => {
      if (event.key !== "Enter" && event.key !== " ") {
        return
      }

      event.preventDefault()
      onTabClick(tab.id)
    },
    [onTabClick, tab.id]
  )

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <div
          ref={setNodeRef}
          className={`tab-item ${isActive ? "active" : ""} ${tab.isModified ? "modified" : ""} ${isDragging ? "dragging" : ""}`}
          role="tab"
          tabIndex={0}
          aria-selected={isActive}
          data-tab-id={tab.id}
          data-allow-context-menu
          onClick={() => onTabClick(tab.id)}
          onContextMenu={handleContextMenu}
          onKeyDown={handleKeyDown}
          onMouseDown={keepFocusOnMouseDown}
          onPointerDown={(event) => onPointerDown(event, tab.id)}
        >
          <span className="tab-number">{index + 1}</span>
          <TabIcon tab={tab} className="tab-icon" />
          <TabAttentionDot tabId={tab.id} />
          <TabLogIndicator tabId={tab.id} />
          <TabTitle tab={tab} className="tab-title" />
          <button
            className="tab-close"
            aria-label={t("contextMenu.closeTab", { defaultValue: "Close tab" })}
            onClick={(e) => {
              e.stopPropagation()
              onTabClose(tab.id)
            }}
            onMouseEnter={(e) => e.stopPropagation()}
            onMouseLeave={(e) => e.stopPropagation()}
          >
            <X size={12} />
          </button>
        </div>
      </TooltipTrigger>
      <TooltipContent>{tabTooltip(tab)}</TooltipContent>
    </Tooltip>
  )
})
