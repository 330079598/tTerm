import "@/components/TabBar.css"
import React, { useCallback, useEffect, useRef, useState } from "react"
import { createPortal } from "react-dom"
import { useTranslation } from "react-i18next"
import { ChevronDown, ChevronLeft, ChevronRight, Search, Settings, X } from "lucide-react"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { Tab, TabContextMenuAction } from "@/types/tab"

const TAB_OVERFLOW_THRESHOLD = 16
const OVERFLOW_PANEL_MAX_WIDTH = 320
const OVERFLOW_PANEL_VIEWPORT_RATIO = 0.7
const OVERFLOW_PANEL_MARGIN = 8
const TAB_DRAG_THRESHOLD = 6

type OverflowPanelStyle = React.CSSProperties & {
  "--tab-overflow-panel-max-height"?: string
}

function getConnectionHostLabel(tab: Tab): string | undefined {
  const host = tab.remoteFile?.host?.trim() || tab.connection?.host?.trim()
  if (!host) {
    return undefined
  }

  const port = tab.connection?.port
  return port && port !== 22 ? `${host}:${port}` : host
}

function getOverflowConnectionMeta(tab: Tab): { primary?: string; secondary?: string } {
  const hostLabel = getConnectionHostLabel(tab)

  if (tab.type !== "remote-file-editor") {
    return { primary: hostLabel }
  }

  const savedName =
    tab.remoteFile?.profileName?.trim() ||
    tab.connection?.profileName?.trim() ||
    tab.remoteFile?.connectionLabel?.trim()

  if (!savedName) {
    return { primary: hostLabel }
  }

  return {
    primary: savedName,
    secondary: hostLabel && hostLabel !== savedName ? hostLabel : undefined,
  }
}

interface TabBarProps {
  tabs: Tab[]
  activeTabId: string | null
  onTabClick: (id: string) => void
  onTabClose: (id: string) => void
  onNewTab: () => void
  onTabMove: (fromIndex: number, toIndex: number) => void
  onTabDragMove: (tabId: string, clientX: number, clientY: number) => void
  onTabDrop: (tabId: string, clientX: number, clientY: number) => boolean
  onTabDragCancel: () => void
  onContextMenu: (event: React.MouseEvent, tab: Tab, actions: TabContextMenuAction[]) => void
}

interface TabItemProps {
  tab: Tab
  index: number
  isActive: boolean
  isDragging: boolean
  isDropTarget: boolean
  setActiveNode?: (node: HTMLDivElement | null) => void
  onTabClick: (id: string) => void
  onTabClose: (id: string) => void
  onContextMenu: (event: React.MouseEvent, tab: Tab, actions: TabContextMenuAction[]) => void
  onPointerDown: (event: React.PointerEvent<HTMLDivElement>, tabId: string) => void
  onPointerMove: (event: React.PointerEvent<HTMLDivElement>) => void
  onPointerUp: (event: React.PointerEvent<HTMLDivElement>) => void
  onPointerCancel: () => void
}

type TabDragState = {
  tabId: string
  pointerId: number
  startX: number
  startY: number
  dragging: boolean
  targetTabId: string | null
}

const TabItem: React.FC<TabItemProps> = ({
  tab,
  index,
  isActive,
  isDragging,
  isDropTarget,
  setActiveNode,
  onTabClick,
  onTabClose,
  onContextMenu,
  onPointerDown,
  onPointerMove,
  onPointerUp,
  onPointerCancel,
}) => {
  const { t } = useTranslation()

  const setNodeRef = (node: HTMLDivElement | null) => {
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
          { label: t("contextMenu.closeTabsToLeft"), action: "close-left" },
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

      const editConnectionAction: TabContextMenuAction | null =
        tab.type === "ssh" && tab.connection?.profileId
          ? { label: t("contextMenu.editConnection"), action: "edit-connection", icon: "edit" }
          : null

      const actions: TabContextMenuAction[] = [
        { label: t("contextMenu.newTab"), action: "new", icon: "plus" },
        { label: t("contextMenu.duplicateTab"), action: "duplicate", icon: "copy" },
        ...(tab.type === "terminal" || tab.type === "ssh"
          ? [
              {
                label: t("contextMenu.splitRight", { defaultValue: "Split Right" }),
                action: "split-right",
                icon: "split-right",
              },
              {
                label: t("contextMenu.splitDown", { defaultValue: "Split Down" }),
                action: "split-down",
                icon: "split-down",
              },
            ]
          : []),
        { label: t("contextMenu.renameTab"), action: "rename", icon: "edit" },
        ...(editConnectionAction ? [editConnectionAction] : []),
        ...(pinAction ? [pinAction] : []),
        { separator: true, label: "", action: "" },
        { label: t("contextMenu.closeTab"), action: "close", icon: "x" },
        { label: t("contextMenu.closeOtherTabs"), action: "close-others" },
        { label: t("contextMenu.closeTabsToLeft"), action: "close-left" },
        { label: t("contextMenu.closeTabsToRight"), action: "close-right" },
      ]
      onContextMenu(e, tab, actions)
    },
    [tab, onContextMenu, t]
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
          className={`tab-item ${isActive ? "active" : ""} ${tab.isModified ? "modified" : ""} ${isDragging ? "dragging" : ""} ${isDropTarget ? "drop-target" : ""}`}
          role="tab"
          tabIndex={0}
          aria-selected={isActive}
          data-tab-id={tab.id}
          data-allow-context-menu
          onClick={() => onTabClick(tab.id)}
          onContextMenu={handleContextMenu}
          onKeyDown={handleKeyDown}
          onPointerDown={(event) => onPointerDown(event, tab.id)}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
          onPointerCancel={onPointerCancel}
        >
          <span className="tab-number">{index + 1}</span>
          {tab.type === "settings" && <Settings className="tab-icon" size={13} />}
          <span className="tab-title">{tab.title}</span>
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
      <TooltipContent>{`${tab.title}${tab.connection ? ` (${tab.connection.host})` : ""}`}</TooltipContent>
    </Tooltip>
  )
}

export const TabBar: React.FC<TabBarProps> = ({
  tabs,
  activeTabId,
  onTabClick,
  onTabClose,
  onTabMove,
  onTabDragMove,
  onTabDrop,
  onTabDragCancel,
  onContextMenu,
}) => {
  const activeTabRef = useRef<HTMLDivElement | null>(null)
  const listRef = useRef<HTMLDivElement | null>(null)
  const overflowMenuRef = useRef<HTMLDivElement | null>(null)
  const overflowTriggerRef = useRef<HTMLButtonElement | null>(null)
  const overflowPanelRef = useRef<HTMLDivElement | null>(null)
  const searchInputRef = useRef<HTMLInputElement | null>(null)
  const dragStateRef = useRef<TabDragState | null>(null)
  const suppressNextClickRef = useRef(false)
  const [scrollState, setScrollState] = useState({ canScrollLeft: false, canScrollRight: false })
  const [isOverflowMenuOpen, setIsOverflowMenuOpen] = useState(false)
  const [searchQuery, setSearchQuery] = useState("")
  const [overflowPanelStyle, setOverflowPanelStyle] = useState<OverflowPanelStyle | null>(null)
  const [draggingTabId, setDraggingTabId] = useState<string | null>(null)
  const [dropTargetTabId, setDropTargetTabId] = useState<string | null>(null)

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

  const resetTabDrag = useCallback(() => {
    dragStateRef.current = null
    setDraggingTabId(null)
    setDropTargetTabId(null)
    onTabDragCancel()
  }, [onTabDragCancel])

  const handleTabPointerDown = useCallback(
    (event: React.PointerEvent<HTMLDivElement>, tabId: string) => {
      const target = event.target
      if (event.button !== 0 || !(target instanceof Element) || target.closest("button")) {
        return
      }

      dragStateRef.current = {
        tabId,
        pointerId: event.pointerId,
        startX: event.clientX,
        startY: event.clientY,
        dragging: false,
        targetTabId: null,
      }
      event.currentTarget.setPointerCapture(event.pointerId)
    },
    []
  )

  const handleTabPointerMove = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      const dragState = dragStateRef.current
      if (!dragState || dragState.pointerId !== event.pointerId) {
        return
      }

      const deltaX = event.clientX - dragState.startX
      const deltaY = event.clientY - dragState.startY

      if (!dragState.dragging) {
        if (Math.hypot(deltaX, deltaY) < TAB_DRAG_THRESHOLD) {
          return
        }
        dragState.dragging = true
        suppressNextClickRef.current = true
        setDraggingTabId(dragState.tabId)
      }

      event.preventDefault()
      onTabDragMove(dragState.tabId, event.clientX, event.clientY)

      const tabElements = Array.from(
        listRef.current?.querySelectorAll<HTMLElement>("[data-tab-id]") ?? []
      )
      const targetTab = tabElements.find((element) => {
        const rect = element.getBoundingClientRect()
        return (
          element.dataset.tabId !== dragState.tabId &&
          event.clientX >= rect.left &&
          event.clientX <= rect.right &&
          event.clientY >= rect.top &&
          event.clientY <= rect.bottom
        )
      })
      const targetTabId = targetTab?.dataset.tabId ?? null

      dragState.targetTabId = targetTabId && targetTabId !== dragState.tabId ? targetTabId : null
      setDropTargetTabId(dragState.targetTabId)
    },
    [onTabDragMove]
  )

  const finishTabDrag = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      const dragState = dragStateRef.current
      if (!dragState || dragState.pointerId !== event.pointerId) {
        return
      }

      if (event.currentTarget.hasPointerCapture(event.pointerId)) {
        event.currentTarget.releasePointerCapture(event.pointerId)
      }

      const handledWorkspaceDrop =
        dragState.dragging && onTabDrop(dragState.tabId, event.clientX, event.clientY)

      if (!handledWorkspaceDrop && dragState.dragging && dragState.targetTabId) {
        event.preventDefault()
        const fromIndex = tabs.findIndex((tab) => tab.id === dragState.tabId)
        const toIndex = tabs.findIndex((tab) => tab.id === dragState.targetTabId)

        if (fromIndex >= 0 && toIndex >= 0 && fromIndex !== toIndex) {
          onTabMove(fromIndex, toIndex)
        }
      }

      resetTabDrag()
    },
    [onTabDrop, onTabMove, resetTabDrag, tabs]
  )

  const handleTabClick = useCallback(
    (id: string) => {
      if (suppressNextClickRef.current) {
        suppressNextClickRef.current = false
        return
      }

      onTabClick(id)
    },
    [onTabClick]
  )

  const handleSelectTab = useCallback(
    (id: string) => {
      onTabClick(id)
      closeOverflowMenu()
    },
    [closeOverflowMenu, onTabClick]
  )

  const hasOverflow = scrollState.canScrollLeft || scrollState.canScrollRight

  const scrollTabs = useCallback(
    (direction: "left" | "right") => {
      const list = listRef.current
      if (!list) {
        return
      }

      const distance = Math.max(140, Math.floor(list.clientWidth * 0.72))
      list.scrollBy({
        left: direction === "left" ? -distance : distance,
        behavior: "smooth",
      })

      window.setTimeout(updateScrollState, 180)
    },
    [updateScrollState]
  )

  const handleTabListWheel = useCallback(
    (event: React.WheelEvent<HTMLDivElement>) => {
      const list = listRef.current
      if (!list || list.scrollWidth <= list.clientWidth) {
        return
      }

      const horizontalDelta =
        Math.abs(event.deltaX) > Math.abs(event.deltaY) ? event.deltaX : event.deltaY

      if (horizontalDelta === 0) {
        return
      }

      event.preventDefault()
      list.scrollLeft += horizontalDelta
      updateScrollState()
    },
    [updateScrollState]
  )

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
          tab.remoteFile?.profileName,
          tab.remoteFile?.connectionLabel,
          tab.remoteFile?.host,
          tab.remoteFile?.path,
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
      {hasOverflow && (
        <button
          type="button"
          className="tab-action tab-scroll-button tab-scroll-left"
          aria-label="Scroll tabs left"
          disabled={!scrollState.canScrollLeft}
          onClick={() => scrollTabs("left")}
        >
          <ChevronLeft size={15} />
        </button>
      )}

      <div
        className={`tab-list-viewport ${scrollState.canScrollLeft ? "can-scroll-left" : ""} ${scrollState.canScrollRight ? "can-scroll-right" : ""}`}
      >
        <div
          ref={listRef}
          className="tab-list"
          onScroll={updateScrollState}
          onWheel={handleTabListWheel}
        >
          {tabs.map((tab, index) => (
            <React.Fragment key={tab.id}>
              <TabItem
                tab={tab}
                index={index}
                isActive={tab.id === activeTabId}
                isDragging={tab.id === draggingTabId}
                isDropTarget={tab.id === dropTargetTabId}
                setActiveNode={tab.id === activeTabId ? setActiveTabNode : undefined}
                onTabClick={handleTabClick}
                onTabClose={onTabClose}
                onContextMenu={onContextMenu}
                onPointerDown={handleTabPointerDown}
                onPointerMove={handleTabPointerMove}
                onPointerUp={finishTabDrag}
                onPointerCancel={resetTabDrag}
              />
            </React.Fragment>
          ))}
        </div>
      </div>

      {hasOverflow && (
        <button
          type="button"
          className="tab-action tab-scroll-button tab-scroll-right"
          aria-label="Scroll tabs right"
          disabled={!scrollState.canScrollRight}
          onClick={() => scrollTabs("right")}
        >
          <ChevronRight size={15} />
        </button>
      )}

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
                    const connectionMeta = getOverflowConnectionMeta(tab)

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
                        {connectionMeta.primary && (
                          <span className="tab-overflow-meta">
                            <span className="tab-overflow-meta-primary">
                              {connectionMeta.primary}
                            </span>
                            {connectionMeta.secondary && (
                              <span className="tab-overflow-meta-secondary">
                                {connectionMeta.secondary}
                              </span>
                            )}
                          </span>
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
