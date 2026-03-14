import React, { useCallback } from "react"
import { DragDropProvider, useDraggable, useDroppable } from "@dnd-kit/react"
import { FolderOpen, Server, Terminal, X, Zap } from "lucide-react"
import { Tab, TabContextMenuAction } from "../types/tab"

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
  isActive: boolean
  onTabClick: (id: string) => void
  onTabClose: (id: string) => void
  onContextMenu: (event: React.MouseEvent, tab: Tab, actions: TabContextMenuAction[]) => void
}

const getTabIcon = (type: Tab["type"]) => {
  switch (type) {
    case "terminal":
      return <Terminal className="tab-icon" />
    case "ssh":
      return <Server className="tab-icon" />
    case "sftp":
      return <FolderOpen className="tab-icon" />
    case "serial":
      return <Zap className="tab-icon" />
    default:
      return <Terminal className="tab-icon" />
  }
}

const TabItem: React.FC<TabItemProps> = ({
  tab,
  isActive,
  onTabClick,
  onTabClose,
  onContextMenu,
}) => {
  // Use stable IDs based on tab.id so multiple拖拽不会失效
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
        { label: "New Tab", action: "new", icon: "plus" },
        { label: "Duplicate Tab", action: "duplicate", icon: "copy" },
        { separator: true, label: "", action: "" },
        { label: "Close Tab", action: "close", icon: "x" },
        { label: "Close Other Tabs", action: "close-others" },
        { label: "Close Tabs to the Right", action: "close-right" },
      ]
      onContextMenu(e, tab, actions)
    },
    [tab, onContextMenu]
  )

  return (
    <div
      ref={setNodeRef}
      className={`tab ${isActive ? "active" : ""} ${tab.isModified ? "modified" : ""} ${isDragging ? "dragging" : ""} ${isDropTarget ? "drop-over" : ""}`}
      onClick={() => onTabClick(tab.id)}
      onContextMenu={handleContextMenu}
      title={`${tab.title}${tab.connection ? ` (${tab.connection.host})` : ""}`}
    >
      {getTabIcon(tab.type)}
      <span className="tab-title">{tab.title}</span>
      <button
        className="tab-close"
        onClick={(e) => {
          e.stopPropagation()
          onTabClose(tab.id)
        }}
        title="Close tab"
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
        {tabs.map((tab) => (
          <React.Fragment key={tab.id}>
            <TabItem
              tab={tab}
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
