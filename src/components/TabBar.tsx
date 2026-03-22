import "@/components/TabBar.css"
import React, { useCallback } from "react"
import { useTranslation } from "react-i18next"
import { DragDropProvider, useDraggable, useDroppable } from "@dnd-kit/react"
import { X } from "lucide-react"
import { Tab, TabContextMenuAction } from "@/types/tab"

interface TabBarProps {
  tabs: Tab[]
  activeTabId: string | null
  onTabClick: (id: string) => void
  onTabClose: (id: string) => void
  onNewTab: () => void
  onTabMove: (fromIndex: number, toIndex: number) => void
  onContextMenu: (event: React.MouseEvent, tab: Tab, actions: TabContextMenuAction[]) => void
}

interface TabItemProps {
  tab: Tab
  index: number
  isActive: boolean
  onTabClick: (id: string) => void
  onTabClose: (id: string) => void
  onContextMenu: (event: React.MouseEvent, tab: Tab, actions: TabContextMenuAction[]) => void
}

const TabItem: React.FC<TabItemProps> = ({
  tab,
  index,
  isActive,
  onTabClick,
  onTabClose,
  onContextMenu,
}) => {
  const { t } = useTranslation()
  // Use stable IDs based on tab.id so multiple drag operations won't fail
  const draggableId = `tab:${tab.id}:drag`
  const droppableId = `tab:${tab.id}:drop`

  const { ref: draggableRef, isDragging } = useDraggable({
    id: draggableId,
  })

  const { ref: droppableRef, isDropTarget } = useDroppable({
    id: droppableId,
  })

  const setNodeRef = (node: HTMLDivElement | null) => {
    draggableRef(node)
    droppableRef(node)
  }

  const handleContextMenu = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault()
      const actions: TabContextMenuAction[] = [
        { label: t("contextMenu.newTab"), action: "new", icon: "plus" },
        { label: t("contextMenu.duplicateTab"), action: "duplicate", icon: "copy" },
        { label: t("contextMenu.renameTab"), action: "rename", icon: "edit" },
        { separator: true, label: "", action: "" },
        { label: t("contextMenu.closeTab"), action: "close", icon: "x" },
        { label: t("contextMenu.closeOtherTabs"), action: "close-others" },
        { label: t("contextMenu.closeTabsToRight"), action: "close-right" },
      ]
      onContextMenu(e, tab, actions)
    },
    [tab, onContextMenu, t]
  )

  return (
    <div
      ref={setNodeRef}
      className={`tab-item ${isActive ? "active" : ""} ${tab.isModified ? "modified" : ""} ${isDragging ? "dragging" : ""} ${isDropTarget ? "drop-target" : ""}`}
      onClick={() => onTabClick(tab.id)}
      onContextMenu={handleContextMenu}
      title={`${tab.title}${tab.connection ? ` (${tab.connection.host})` : ""}`}
    >
      <span className="tab-number">{index + 1}</span>
      <span className="tab-title">{tab.title}</span>
      <button
        className="tab-close"
        onClick={(e) => {
          e.stopPropagation()
          onTabClose(tab.id)
        }}
        title={t("tabs.closeTab")}
      >
        <X size={12} />
      </button>
    </div>
  )
}

const getTabIdFromDndId = (id?: string | number | null): string | null => {
  if (!id) return null
  const parts = String(id).split(":")
  // Expect format "tab:{tabId}:drag|drop"
  if (parts.length < 3 || parts[0] !== "tab") return null
  return parts[1] || null
}

export const TabBar: React.FC<TabBarProps> = ({
  tabs,
  activeTabId,
  onTabClick,
  onTabClose,
  onTabMove,
  onContextMenu,
}) => {
  const handleDragEnd = useCallback(
    (
      event: Parameters<NonNullable<React.ComponentProps<typeof DragDropProvider>["onDragEnd"]>>[0]
    ) => {
      if (event.canceled) return

      const sourceTabId = getTabIdFromDndId(event.operation.source?.id ?? null)
      const targetTabId = getTabIdFromDndId(event.operation.target?.id ?? null)

      if (!sourceTabId || !targetTabId || sourceTabId === targetTabId) {
        return
      }

      const fromIndex = tabs.findIndex((t) => t.id === sourceTabId)
      const toIndex = tabs.findIndex((t) => t.id === targetTabId)

      if (fromIndex >= 0 && toIndex >= 0 && fromIndex !== toIndex) {
        onTabMove(fromIndex, toIndex)
      }
    },
    [onTabMove, tabs]
  )

  return (
    <div className="tab-list">
      <DragDropProvider onDragEnd={handleDragEnd}>
        {tabs.map((tab, index) => (
          <React.Fragment key={tab.id}>
            <TabItem
              tab={tab}
              index={index}
              isActive={tab.id === activeTabId}
              onTabClick={onTabClick}
              onTabClose={onTabClose}
              onContextMenu={onContextMenu}
            />
          </React.Fragment>
        ))}
      </DragDropProvider>
    </div>
  )
}
