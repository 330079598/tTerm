import "@/components/TabBar.css"
import React, { useCallback, useEffect, useRef, useState } from "react"
import { createPortal } from "react-dom"
import { useTranslation } from "react-i18next"
import { DragDropProvider, useDraggable, useDroppable } from "@dnd-kit/react"
import { ChevronDown, Search, Settings, X } from "lucide-react"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { Tab, TabContextMenuAction } from "@/types/tab"

const TAB_OVERFLOW_THRESHOLD = 16
const OVERFLOW_PANEL_MAX_WIDTH = 320
const OVERFLOW_PANEL_VIEWPORT_RATIO = 0.7
const OVERFLOW_PANEL_MARGIN = 8

type OverflowPanelStyle = React.CSSProperties & {
  "--tab-overflow-panel-max-height"?: string
}

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
  setActiveNode?: (node: HTMLDivElement | null) => void
  onTabClick: (id: string) => void
  onTabClose: (id: string) => void
  onContextMenu: (event: React.MouseEvent, tab: Tab, actions: TabContextMenuAction[]) => void
}

const TabItem: React.FC<TabItemProps> = ({
  tab,
  index,
  isActive,
  setActiveNode,
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
    if (isActive && setActiveNode) {
      setActiveNode(node)
    }
  }

  const handleContextMenu = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault()

      if (tab.type === "settings") {
        onContextMenu(e, tab, [
          { label: t("contextMenu.newTab"), action: "new", icon: "plus" },
          { separator: true, label: "", action: "" },
          { label: t("contextMenu.closeTab"), action: "close", icon: "x" },
          { label: t("contextMenu.closeOtherTabs"), action: "close-others" },
          { label: t("contextMenu.closeTabsToRight"), action: "close-right" },
        ])
        return
      }

      const pinAction: TabContextMenuAction | null =
        tab.type === "ssh"
          ? tab.connectionHeaderPinned === false
            ? { label: t("contextMenu.pinConnectionHeader"), action: "pin-header", icon: "pin" }
            : {
                label: t("contextMenu.unpinConnectionHeader"),
                action: "unpin-header",
                icon: "pin-off",
              }
          : null

      const actions: TabContextMenuAction[] = [
        { label: t("contextMenu.newTab"), action: "new", icon: "plus" },
        { label: t("contextMenu.duplicateTab"), action: "duplicate", icon: "copy" },
        { label: t("contextMenu.renameTab"), action: "rename", icon: "edit" },
        ...(pinAction ? [pinAction] : []),
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
    <Tooltip>
      <TooltipTrigger asChild>
        <div
          ref={setNodeRef}
          className={`tab-item ${isActive ? "active" : ""} ${tab.isModified ? "modified" : ""} ${isDragging ? "dragging" : ""} ${isDropTarget ? "drop-target" : ""}`}
          onClick={() => onTabClick(tab.id)}
          onContextMenu={handleContextMenu}
        >
          <span className="tab-number">{index + 1}</span>
          {tab.type === "settings" && <Settings className="tab-icon" size={13} />}
          <span className="tab-title">{tab.title}</span>
          <button
            className="tab-close"
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
      <TooltipContent>{`${tab.title}${tab.connection ? ` (${tab.connection.host})` : ""}`}</TooltipContent>
    </Tooltip>
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
  const activeTabRef = useRef<HTMLDivElement | null>(null)
  const listRef = useRef<HTMLDivElement | null>(null)
  const overflowMenuRef = useRef<HTMLDivElement | null>(null)
  const overflowTriggerRef = useRef<HTMLButtonElement | null>(null)
  const overflowPanelRef = useRef<HTMLDivElement | null>(null)
  const searchInputRef = useRef<HTMLInputElement | null>(null)
  const [scrollState, setScrollState] = useState({ canScrollLeft: false, canScrollRight: false })
  const [isOverflowMenuOpen, setIsOverflowMenuOpen] = useState(false)
  const [searchQuery, setSearchQuery] = useState("")
  const [overflowPanelStyle, setOverflowPanelStyle] = useState<OverflowPanelStyle | null>(null)

  const updateScrollState = useCallback(() => {
    const list = listRef.current
    if (!list) {
      setScrollState({ canScrollLeft: false, canScrollRight: false })
      return
    }

    const maxScrollLeft = list.scrollWidth - list.clientWidth
    setScrollState({
      canScrollLeft: list.scrollLeft > TAB_OVERFLOW_THRESHOLD,
      canScrollRight: maxScrollLeft - list.scrollLeft > TAB_OVERFLOW_THRESHOLD,
    })
  }, [])

  const setActiveTabNode = useCallback((node: HTMLDivElement | null) => {
    activeTabRef.current = node
  }, [])

  useEffect(() => {
    const timer = setTimeout(() => {
      activeTabRef.current?.scrollIntoView({
        behavior: "smooth",
        block: "nearest",
        inline: "nearest",
      })
    }, 50)
    return () => clearTimeout(timer)
  }, [activeTabId])

  useEffect(() => {
    const timer = setTimeout(updateScrollState, 80)
    return () => clearTimeout(timer)
  }, [tabs, activeTabId, updateScrollState])

  useEffect(() => {
    const list = listRef.current
    if (!list) {
      return
    }

    const resizeObserver = new ResizeObserver(updateScrollState)
    resizeObserver.observe(list)
    const animationFrame = requestAnimationFrame(updateScrollState)

    return () => {
      cancelAnimationFrame(animationFrame)
      resizeObserver.disconnect()
    }
  }, [updateScrollState])

  const updateOverflowPanelPosition = useCallback(() => {
    const trigger = overflowTriggerRef.current
    if (!trigger) {
      return
    }

    const triggerRect = trigger.getBoundingClientRect()
    const viewportMargin = Math.min(OVERFLOW_PANEL_MARGIN, window.innerWidth / 2)
    const availableWidth = Math.max(0, window.innerWidth - viewportMargin * 2)
    const panelWidth = Math.min(
      OVERFLOW_PANEL_MAX_WIDTH,
      window.innerWidth * OVERFLOW_PANEL_VIEWPORT_RATIO,
      availableWidth
    )
    const left = Math.min(
      Math.max(viewportMargin, triggerRect.right - panelWidth),
      window.innerWidth - panelWidth - viewportMargin
    )
    const top = triggerRect.bottom + 4
    const maxHeight = Math.max(120, Math.min(420, window.innerHeight - top - viewportMargin))

    setOverflowPanelStyle({
      left,
      top,
      width: panelWidth,
      maxHeight,
      "--tab-overflow-panel-max-height": `${maxHeight}px`,
    })
  }, [])

  const closeOverflowMenu = useCallback(() => {
    setIsOverflowMenuOpen(false)
    setSearchQuery("")
    setOverflowPanelStyle(null)
  }, [])

  useEffect(() => {
    if (!isOverflowMenuOpen) {
      return
    }

    searchInputRef.current?.focus()

    const handlePointerDown = (event: MouseEvent) => {
      const target = event.target as Node
      if (
        !overflowMenuRef.current?.contains(target) &&
        !overflowPanelRef.current?.contains(target)
      ) {
        closeOverflowMenu()
      }
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        closeOverflowMenu()
      }
    }

    document.addEventListener("mousedown", handlePointerDown)
    document.addEventListener("keydown", handleKeyDown)

    return () => {
      document.removeEventListener("mousedown", handlePointerDown)
      document.removeEventListener("keydown", handleKeyDown)
    }
  }, [closeOverflowMenu, isOverflowMenuOpen])

  useEffect(() => {
    if (!isOverflowMenuOpen) {
      return
    }

    window.addEventListener("resize", updateOverflowPanelPosition)
    window.addEventListener("scroll", updateOverflowPanelPosition, true)

    return () => {
      window.removeEventListener("resize", updateOverflowPanelPosition)
      window.removeEventListener("scroll", updateOverflowPanelPosition, true)
    }
  }, [isOverflowMenuOpen, updateOverflowPanelPosition])

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

  const handleSelectTab = useCallback(
    (id: string) => {
      onTabClick(id)
      closeOverflowMenu()
    },
    [closeOverflowMenu, onTabClick]
  )

  const hasOverflow = scrollState.canScrollLeft || scrollState.canScrollRight
  const normalizedSearchQuery = searchQuery.trim().toLowerCase()
  const filteredTabs = normalizedSearchQuery
    ? tabs.filter((tab, index) => {
        const connection = tab.connection
        const searchableText = [
          String(index + 1),
          tab.title,
          tab.type,
          connection?.host,
          connection?.username,
          connection?.profileName,
          connection?.port ? String(connection.port) : undefined,
        ]
          .filter(Boolean)
          .join(" ")
          .toLowerCase()

        return searchableText.includes(normalizedSearchQuery)
      })
    : tabs

  const handleSearchKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLInputElement>) => {
      if (event.key === "Enter" && filteredTabs.length > 0) {
        handleSelectTab(filteredTabs[0].id)
      }
    },
    [filteredTabs, handleSelectTab]
  )

  return (
    <div className="tab-bar-shell">
      <div
        className={`tab-list-viewport ${scrollState.canScrollLeft ? "can-scroll-left" : ""} ${scrollState.canScrollRight ? "can-scroll-right" : ""}`}
      >
        <div ref={listRef} className="tab-list" onScroll={updateScrollState}>
          <DragDropProvider onDragEnd={handleDragEnd}>
            {tabs.map((tab, index) => (
              <React.Fragment key={tab.id}>
                <TabItem
                  tab={tab}
                  index={index}
                  isActive={tab.id === activeTabId}
                  setActiveNode={tab.id === activeTabId ? setActiveTabNode : undefined}
                  onTabClick={onTabClick}
                  onTabClose={onTabClose}
                  onContextMenu={onContextMenu}
                />
              </React.Fragment>
            ))}
          </DragDropProvider>
        </div>
      </div>

      {hasOverflow && (
        <div ref={overflowMenuRef} className="tab-overflow-menu">
          <button
            ref={overflowTriggerRef}
            type="button"
            className="tab-action tab-overflow-trigger"
            aria-expanded={isOverflowMenuOpen}
            aria-label="Show all tabs"
            onClick={() => {
              if (isOverflowMenuOpen) {
                closeOverflowMenu()
                return
              }

              updateOverflowPanelPosition()
              setIsOverflowMenuOpen(true)
            }}
          >
            <ChevronDown size={15} />
          </button>

          {isOverflowMenuOpen &&
            createPortal(
              <div
                ref={overflowPanelRef}
                className="tab-overflow-panel"
                role="dialog"
                aria-label="Search tabs"
                style={overflowPanelStyle ?? undefined}
              >
                <div className="tab-search-box">
                  <Search size={14} />
                  <input
                    ref={searchInputRef}
                    className="tab-search-input"
                    value={searchQuery}
                    placeholder="Search tabs"
                    onChange={(event) => setSearchQuery(event.target.value)}
                    onKeyDown={handleSearchKeyDown}
                  />
                  {searchQuery && (
                    <button
                      type="button"
                      className="tab-search-clear"
                      aria-label="Clear search"
                      onClick={() => {
                        setSearchQuery("")
                        searchInputRef.current?.focus()
                      }}
                    >
                      <X size={13} />
                    </button>
                  )}
                </div>

                <div className="tab-overflow-results">
                  {filteredTabs.map((tab) => {
                    const tabIndex = tabs.findIndex((currentTab) => currentTab.id === tab.id)

                    return (
                      <button
                        key={tab.id}
                        type="button"
                        className={`tab-overflow-item ${tab.id === activeTabId ? "active" : ""}`}
                        onClick={() => handleSelectTab(tab.id)}
                      >
                        <span className="tab-overflow-number">{tabIndex + 1}</span>
                        {tab.type === "settings" && <Settings className="tab-icon" size={13} />}
                        <span className="tab-overflow-title">{tab.title}</span>
                        {tab.connection?.host && (
                          <span className="tab-overflow-host">{tab.connection.host}</span>
                        )}
                      </button>
                    )
                  })}

                  {filteredTabs.length === 0 && (
                    <div className="tab-overflow-empty">No matching tabs</div>
                  )}
                </div>
              </div>,
              document.body
            )}
        </div>
      )}
    </div>
  )
}
