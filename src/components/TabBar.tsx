import "@/components/TabBar.css"
import React, { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react"
import { createPortal } from "react-dom"
import { useTranslation } from "react-i18next"
import {
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  ScrollText,
  Search,
  Settings,
  Waypoints,
  X,
} from "lucide-react"
import { TabAttentionDot } from "@/components/TabAttentionDot"
import { TabLogIndicator } from "@/components/TabLogIndicator"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { isPageTab, Tab, TabContextMenuAction } from "@/types/tab"
import { isImeKeyEvent } from "@/lib/ime"
import {
  focusActiveTerminal,
  keepFocusOnMouseDown,
  refocusTerminalIfFocusLost,
} from "@/lib/terminalFocus"
import { getTabCloseMenuActions } from "@/lib/tabClosing"
import { getTabLogMenuActions } from "@/lib/terminalLogRecording"

const TAB_OVERFLOW_THRESHOLD = 16
const OVERFLOW_PANEL_MAX_WIDTH = 320
const OVERFLOW_PANEL_VIEWPORT_RATIO = 0.7
const OVERFLOW_PANEL_MARGIN = 8
const TAB_DRAG_THRESHOLD = 6
// How far the pointer may stray above or below the tab list before the tab
// detaches from it and becomes a ghost that can be dropped on the workspace.
const TAB_DRAG_DETACH_DISTANCE = 24
const TAB_DRAG_AUTO_SCROLL_EDGE = 40
const TAB_DRAG_AUTO_SCROLL_MAX_SPEED = 14

type OverflowPanelStyle = React.CSSProperties & {
  "--tab-overflow-panel-max-height"?: string
}

// A tab's offsetLeft counts from the list's content box, so pointer positions
// are put on the same footing, independent of how far the list has scrolled.
function getListContentX(list: HTMLElement, clientX: number): number {
  return clientX - list.getBoundingClientRect().left - list.clientLeft + list.scrollLeft
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
  getTabContextIds: (tabId: string) => string[]
  onContextMenu: (event: React.MouseEvent, tab: Tab, actions: TabContextMenuAction[]) => void
}

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

type TabDragMode = "list" | "detached"

type TabDragState = {
  tabId: string
  pointerId: number
  startX: number
  startY: number
  dragging: boolean
  mode: TabDragMode
  sourceElement: HTMLDivElement
  threshold: number
  // Where the pointer grabbed the tab, measured from the tab's left edge.
  grabOffsetX: number
  gap: number
  // The index the tab lands on if dropped now; null while detached.
  toIndex: number | null
}

const TabItem = React.memo(function TabItem({
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
    if (isActive && setActiveNode) {
      setActiveNode(node)
    }
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
        ...(tab.type === "terminal" || tab.type === "ssh"
          ? [
              {
                label: t("contextMenu.splitRight", { defaultValue: "Split Right" }),
                action: "split-right",
                icon: "split-right",
              },
              {
                label: t("contextMenu.splitLeft", { defaultValue: "Split Left" }),
                action: "split-left",
                icon: "split-left",
              },
              {
                label: t("contextMenu.splitDown", { defaultValue: "Split Down" }),
                action: "split-down",
                icon: "split-down",
              },
              {
                label: t("contextMenu.splitAbove", { defaultValue: "Split Above" }),
                action: "split-above",
                icon: "split-above",
              },
            ]
          : []),
        { label: t("contextMenu.renameTab"), action: "rename", icon: "edit" },
        ...(editConnectionAction ? [editConnectionAction] : []),
        ...(pinAction ? [pinAction] : []),
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
          {tab.type === "settings" && <Settings className="tab-icon" size={13} />}
          {tab.type === "tunnels" && <Waypoints className="tab-icon" size={13} />}
          {tab.type === "logs" && <ScrollText className="tab-icon" size={13} />}
          <TabAttentionDot tabId={tab.id} />
          <TabLogIndicator tabId={tab.id} />
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
})

export const TabBar: React.FC<TabBarProps> = ({
  tabs,
  activeTabId,
  onTabClick,
  onTabClose,
  onTabMove,
  onTabDragMove,
  onTabDrop,
  onTabDragCancel,
  getTabContextIds,
  onContextMenu,
}) => {
  const { t } = useTranslation()
  const activeTabRef = useRef<HTMLDivElement | null>(null)
  const listRef = useRef<HTMLDivElement | null>(null)
  const viewportRef = useRef<HTMLDivElement | null>(null)
  const overflowMenuRef = useRef<HTMLDivElement | null>(null)
  const overflowTriggerRef = useRef<HTMLButtonElement | null>(null)
  const overflowPanelRef = useRef<HTMLDivElement | null>(null)
  const searchInputRef = useRef<HTMLInputElement | null>(null)
  const dragStateRef = useRef<TabDragState | null>(null)
  const dragGhostRef = useRef<HTMLDivElement | null>(null)
  const lastPointerPosRef = useRef<{ x: number; y: number } | null>(null)
  const autoScrollRef = useRef<{ frame: number | null; speed: number }>({ frame: null, speed: 0 })
  // Each tab's on-screen left edge as the drag ended, so the next layout can
  // slide every tab from there into its slot instead of jumping (FLIP).
  const pendingFlipRef = useRef<{ lefts: Map<string, number>; raisedTabId: string } | null>(null)
  const tabsRef = useRef(tabs)
  const suppressNextClickRef = useRef(false)
  const [scrollState, setScrollState] = useState({ canScrollLeft: false, canScrollRight: false })
  const [isOverflowMenuOpen, setIsOverflowMenuOpen] = useState(false)
  const [searchQuery, setSearchQuery] = useState("")
  const [overflowPanelStyle, setOverflowPanelStyle] = useState<OverflowPanelStyle | null>(null)
  const [draggingTabId, setDraggingTabId] = useState<string | null>(null)
  const [dragMode, setDragMode] = useState<TabDragMode | null>(null)
  const [dragGhostTab, setDragGhostTab] = useState<Tab | null>(null)

  useEffect(() => {
    tabsRef.current = tabs
  }, [tabs])

  useEffect(() => {
    if (dragGhostTab && dragGhostRef.current && lastPointerPosRef.current) {
      const { x, y } = lastPointerPosRef.current
      dragGhostRef.current.style.transform = `translate(${x}px, ${y}px)`
    }
  }, [dragGhostTab])

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
      refocusTerminalIfFocusLost()
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

  const getTabElements = useCallback(
    () => Array.from(listRef.current?.querySelectorAll<HTMLElement>("[data-tab-id]") ?? []),
    []
  )

  const captureTabPositions = useCallback(
    (raisedTabId: string) => {
      pendingFlipRef.current = {
        lefts: new Map(
          getTabElements().map((element) => [
            element.dataset.tabId ?? "",
            element.getBoundingClientRect().left,
          ])
        ),
        raisedTabId,
      }
    },
    [getTabElements]
  )

  useLayoutEffect(() => {
    const pendingFlip = pendingFlipRef.current
    if (!pendingFlip) {
      return
    }
    pendingFlipRef.current = null
    const previousLefts = pendingFlip.lefts

    const elements = getTabElements()
    for (const element of elements) {
      element.style.transition = "none"
      element.style.transform = ""
    }
    const offsets = elements.map((element) => {
      const previousLeft = previousLefts.get(element.dataset.tabId ?? "")
      return previousLeft === undefined ? 0 : previousLeft - element.getBoundingClientRect().left
    })
    elements.forEach((element, index) => {
      if (Math.abs(offsets[index]) >= 0.5) {
        element.style.transform = `translateX(${offsets[index]}px)`
      }
    })
    // Commit the inverted positions before letting them transition back to 0.
    void listRef.current?.offsetWidth
    for (const element of elements) {
      element.style.transition = ""
      element.style.transform = ""
    }

    // The dropped tab has lost its dragging z-index by now; keep it above its
    // neighbours (the active tab sits raised too) until it has settled.
    const droppedTab = elements.find((element) => element.dataset.tabId === pendingFlip.raisedTabId)
    if (droppedTab && typeof droppedTab.getAnimations === "function") {
      const settling = droppedTab
        .getAnimations()
        .find(
          (animation) =>
            animation instanceof CSSTransition && animation.transitionProperty === "transform"
        )
      if (settling) {
        droppedTab.style.zIndex = "3"
        const lower = () => {
          droppedTab.style.zIndex = ""
        }
        settling.finished.then(lower, lower)
      }
    }
  })

  const stopAutoScroll = useCallback(() => {
    const autoScroll = autoScrollRef.current
    if (autoScroll.frame !== null) {
      cancelAnimationFrame(autoScroll.frame)
    }
    autoScroll.frame = null
    autoScroll.speed = 0
  }, [])

  // Follows the pointer with the dragged tab inside the list, and slides the
  // tabs it passes over out of the way by one tab width plus the gap.
  const layoutListDrag = useCallback(
    (dragState: TabDragState, clientX: number) => {
      const list = listRef.current
      const elements = getTabElements()
      const source = dragState.sourceElement
      const fromIndex = elements.indexOf(source)
      if (!list || fromIndex < 0) {
        return
      }

      const first = elements[0]
      const last = elements[elements.length - 1]
      const offset = Math.min(
        Math.max(
          getListContentX(list, clientX) - dragState.grabOffsetX - source.offsetLeft,
          first.offsetLeft - source.offsetLeft
        ),
        last.offsetLeft + last.offsetWidth - source.offsetLeft - source.offsetWidth
      )
      source.style.transform = `translateX(${offset}px)`

      // A tab is passed once the dragged tab's leading edge crosses its middle.
      // Comparing centers instead would strand a wide tab that cannot travel
      // past the narrower first or last tab, since the drag stops at the ends.
      const draggedLeft = source.offsetLeft + offset
      const draggedRight = draggedLeft + source.offsetWidth
      let toIndex = 0
      elements.forEach((element, index) => {
        const elementCenter = element.offsetLeft + element.offsetWidth / 2
        if (
          (index < fromIndex && elementCenter <= draggedLeft) ||
          (index > fromIndex && elementCenter < draggedRight)
        ) {
          toIndex += 1
        }
      })

      const shift = source.offsetWidth + dragState.gap
      elements.forEach((element, index) => {
        if (index === fromIndex) {
          return
        }
        const elementShift =
          index > fromIndex && index <= toIndex
            ? -shift
            : index < fromIndex && index >= toIndex
              ? shift
              : 0
        element.style.transform = elementShift ? `translateX(${elementShift}px)` : ""
      })
      dragState.toIndex = toIndex
    },
    [getTabElements]
  )

  // Closes the gap the detached tab left behind.
  const layoutDetachedDrag = useCallback(
    (dragState: TabDragState) => {
      const elements = getTabElements()
      const source = dragState.sourceElement
      const fromIndex = elements.indexOf(source)
      const shift = source.offsetWidth + dragState.gap
      elements.forEach((element, index) => {
        element.style.transform =
          fromIndex >= 0 && index > fromIndex ? `translateX(${-shift}px)` : ""
      })
      dragState.toIndex = null
    },
    [getTabElements]
  )

  const updateAutoScroll = useCallback(
    (clientX: number) => {
      const list = listRef.current
      const viewport = viewportRef.current
      const autoScroll = autoScrollRef.current
      if (!list || !viewport || list.scrollWidth <= list.clientWidth) {
        stopAutoScroll()
        return
      }

      const rect = viewport.getBoundingClientRect()
      const edge = TAB_DRAG_AUTO_SCROLL_EDGE
      const leftDepth = rect.left + edge - clientX
      const rightDepth = clientX - (rect.right - edge)
      autoScroll.speed =
        leftDepth > 0
          ? -Math.min(1, leftDepth / edge) * TAB_DRAG_AUTO_SCROLL_MAX_SPEED
          : rightDepth > 0
            ? Math.min(1, rightDepth / edge) * TAB_DRAG_AUTO_SCROLL_MAX_SPEED
            : 0

      if (autoScroll.speed === 0) {
        stopAutoScroll()
        return
      }
      if (autoScroll.frame !== null) {
        return
      }

      const step = () => {
        const dragState = dragStateRef.current
        const pointer = lastPointerPosRef.current
        if (!dragState || dragState.mode !== "list" || !pointer || autoScroll.speed === 0) {
          autoScroll.frame = null
          return
        }
        const scrollLeft = list.scrollLeft
        list.scrollLeft += autoScroll.speed
        if (list.scrollLeft !== scrollLeft) {
          layoutListDrag(dragState, pointer.x)
        }
        autoScroll.frame = requestAnimationFrame(step)
      }
      autoScroll.frame = requestAnimationFrame(step)
    },
    [layoutListDrag, stopAutoScroll]
  )

  const resetTabDrag = useCallback(() => {
    dragStateRef.current = null
    stopAutoScroll()
    setDraggingTabId(null)
    setDragMode(null)
    setDragGhostTab(null)
    onTabDragCancel()
  }, [onTabDragCancel, stopAutoScroll])

  const cancelTabDrag = useCallback(() => {
    const dragState = dragStateRef.current
    if (!dragState) {
      return
    }

    if (dragState.sourceElement.hasPointerCapture(dragState.pointerId)) {
      dragState.sourceElement.releasePointerCapture(dragState.pointerId)
    }
    if (dragState.dragging) {
      captureTabPositions(dragState.tabId)
    }
    resetTabDrag()
  }, [captureTabPositions, resetTabDrag])

  const handleTabPointerDown = useCallback(
    (event: React.PointerEvent<HTMLDivElement>, tabId: string) => {
      const target = event.target
      if (event.button !== 0 || !(target instanceof Element) || target.closest("button")) {
        return
      }

      suppressNextClickRef.current = false
      dragStateRef.current = {
        tabId,
        pointerId: event.pointerId,
        startX: event.clientX,
        startY: event.clientY,
        dragging: false,
        mode: "list",
        sourceElement: event.currentTarget,
        threshold: TAB_DRAG_THRESHOLD * (window.devicePixelRatio || 1),
        // Measured on screen rather than from offsetLeft, so a tab grabbed
        // while still sliding into place stays under the pointer.
        grabOffsetX: event.clientX - event.currentTarget.getBoundingClientRect().left,
        gap: 0,
        toIndex: null,
      }
      event.currentTarget.setPointerCapture(event.pointerId)
    },
    []
  )

  const updateTabDrag = useCallback(
    (dragState: TabDragState, clientX: number, clientY: number) => {
      const listRect = listRef.current?.getBoundingClientRect()
      const mode: TabDragMode =
        listRect &&
        clientY >= listRect.top - TAB_DRAG_DETACH_DISTANCE &&
        clientY <= listRect.bottom + TAB_DRAG_DETACH_DISTANCE
          ? "list"
          : "detached"

      if (mode !== dragState.mode) {
        dragState.mode = mode
        setDragMode(mode)
        if (mode === "detached") {
          setDragGhostTab(tabsRef.current.find((tab) => tab.id === dragState.tabId) ?? null)
        } else {
          setDragGhostTab(null)
          onTabDragCancel()
        }
      }

      if (mode === "list") {
        layoutListDrag(dragState, clientX)
        updateAutoScroll(clientX)
        return
      }

      stopAutoScroll()
      layoutDetachedDrag(dragState)
      const ghost = dragGhostRef.current
      if (ghost) {
        ghost.style.transform = `translate(${clientX}px, ${clientY}px)`
      }
      onTabDragMove(dragState.tabId, clientX, clientY)
    },
    [
      layoutDetachedDrag,
      layoutListDrag,
      onTabDragCancel,
      onTabDragMove,
      stopAutoScroll,
      updateAutoScroll,
    ]
  )

  const handleTabPointerMove = useCallback(
    (event: PointerEvent) => {
      const dragState = dragStateRef.current
      if (!dragState || dragState.pointerId !== event.pointerId) {
        return
      }

      const deltaX = event.clientX - dragState.startX
      const deltaY = event.clientY - dragState.startY
      lastPointerPosRef.current = { x: event.clientX, y: event.clientY }

      if (!dragState.dragging) {
        if (Math.hypot(deltaX, deltaY) < dragState.threshold) {
          return
        }
        dragState.dragging = true
        dragState.gap = listRef.current
          ? parseFloat(getComputedStyle(listRef.current).columnGap) || 0
          : 0
        suppressNextClickRef.current = true
        setDraggingTabId(dragState.tabId)
        setDragMode("list")
      }

      event.preventDefault()
      updateTabDrag(dragState, event.clientX, event.clientY)
    },
    [updateTabDrag]
  )

  const finishTabDrag = useCallback(
    (event: PointerEvent) => {
      const dragState = dragStateRef.current
      if (!dragState || dragState.pointerId !== event.pointerId) {
        return
      }

      if (dragState.sourceElement.hasPointerCapture(event.pointerId)) {
        dragState.sourceElement.releasePointerCapture(event.pointerId)
      }

      if (dragState.dragging) {
        event.preventDefault()
        if (dragState.mode === "detached") {
          onTabDrop(dragState.tabId, event.clientX, event.clientY)
        }

        captureTabPositions(dragState.tabId)
        const fromIndex = tabsRef.current.findIndex((tab) => tab.id === dragState.tabId)
        const toIndex = dragState.toIndex
        if (fromIndex >= 0 && toIndex !== null && fromIndex !== toIndex) {
          onTabMove(fromIndex, toIndex)
        }
      }

      resetTabDrag()
    },
    [captureTabPositions, onTabDrop, onTabMove, resetTabDrag]
  )

  const handleTabListScroll = useCallback(() => {
    updateScrollState()
    const dragState = dragStateRef.current
    const pointer = lastPointerPosRef.current
    if (dragState?.dragging && dragState.mode === "list" && pointer) {
      layoutListDrag(dragState, pointer.x)
    }
  }, [layoutListDrag, updateScrollState])

  useEffect(() => {
    const handlePointerCancel = (event: PointerEvent) => {
      if (dragStateRef.current?.pointerId === event.pointerId) {
        cancelTabDrag()
      }
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && dragStateRef.current?.dragging) {
        event.preventDefault()
        event.stopPropagation()
        cancelTabDrag()
      }
    }

    window.addEventListener("pointermove", handleTabPointerMove, true)
    window.addEventListener("pointerup", finishTabDrag, true)
    window.addEventListener("pointercancel", handlePointerCancel, true)
    window.addEventListener("keydown", handleKeyDown, true)
    window.addEventListener("blur", cancelTabDrag)

    return () => {
      window.removeEventListener("pointermove", handleTabPointerMove, true)
      window.removeEventListener("pointerup", finishTabDrag, true)
      window.removeEventListener("pointercancel", handlePointerCancel, true)
      window.removeEventListener("keydown", handleKeyDown, true)
      window.removeEventListener("blur", cancelTabDrag)
    }
  }, [cancelTabDrag, finishTabDrag, handleTabPointerMove])

  useEffect(() => stopAutoScroll, [stopAutoScroll])

  const handleTabClick = useCallback(
    (id: string) => {
      if (suppressNextClickRef.current) {
        suppressNextClickRef.current = false
        return
      }

      onTabClick(id)
      // Switching tabs focuses the new one's terminal; clicking the tab
      // already in sight is a way back to its terminal too.
      if (id === activeTabId) focusActiveTerminal()
    },
    [activeTabId, onTabClick]
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
    (event: WheelEvent) => {
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

  useEffect(() => {
    const list = listRef.current
    if (!list) return

    list.addEventListener("wheel", handleTabListWheel, { passive: false })
    return () => list.removeEventListener("wheel", handleTabListWheel)
  }, [handleTabListWheel])

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
      if (event.key === "Enter" && !isImeKeyEvent(event.nativeEvent) && filteredTabs.length > 0) {
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
          onMouseDown={keepFocusOnMouseDown}
          aria-label={t("tabs.scrollLeft", { defaultValue: "Scroll tabs left" })}
          disabled={!scrollState.canScrollLeft}
          onClick={() => scrollTabs("left")}
        >
          <ChevronLeft size={15} />
        </button>
      )}

      <div
        ref={viewportRef}
        className={`tab-list-viewport ${scrollState.canScrollLeft ? "can-scroll-left" : ""} ${scrollState.canScrollRight ? "can-scroll-right" : ""}`}
      >
        <div
          ref={listRef}
          className={`tab-list ${dragMode ? "sorting" : ""} ${dragMode === "detached" ? "detached" : ""}`}
          onScroll={handleTabListScroll}
        >
          {tabs.map((tab, index) => (
            <React.Fragment key={tab.id}>
              <TabItem
                tab={tab}
                index={index}
                getTabContextIds={getTabContextIds}
                isActive={tab.id === activeTabId}
                isDragging={tab.id === draggingTabId}
                setActiveNode={tab.id === activeTabId ? setActiveTabNode : undefined}
                onTabClick={handleTabClick}
                onTabClose={onTabClose}
                onContextMenu={onContextMenu}
                onPointerDown={handleTabPointerDown}
              />
            </React.Fragment>
          ))}
        </div>
      </div>

      {hasOverflow && (
        <button
          type="button"
          className="tab-action tab-scroll-button tab-scroll-right"
          onMouseDown={keepFocusOnMouseDown}
          aria-label={t("tabs.scrollRight", { defaultValue: "Scroll tabs right" })}
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
            onMouseDown={keepFocusOnMouseDown}
            aria-expanded={isOverflowMenuOpen}
            aria-label={t("tabs.showAll", { defaultValue: "Show all tabs" })}
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
                aria-label={t("tabs.searchTabs", { defaultValue: "Search tabs" })}
                style={overflowPanelStyle ?? undefined}
              >
                <div className="tab-search-box">
                  <Search size={14} />
                  <input
                    ref={searchInputRef}
                    className="tab-search-input"
                    value={searchQuery}
                    placeholder={t("tabs.searchTabs", { defaultValue: "Search tabs" })}
                    onChange={(event) => setSearchQuery(event.target.value)}
                    onKeyDown={handleSearchKeyDown}
                  />
                  {searchQuery && (
                    <button
                      type="button"
                      className="tab-search-clear"
                      aria-label={t("tabs.clearSearch", { defaultValue: "Clear search" })}
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
                        {tab.type === "tunnels" && <Waypoints className="tab-icon" size={13} />}
                        {tab.type === "logs" && <ScrollText className="tab-icon" size={13} />}
                        <TabAttentionDot tabId={tab.id} />
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
                    <div className="tab-overflow-empty">
                      {t("tabs.noMatchingTabs", { defaultValue: "No matching tabs" })}
                    </div>
                  )}
                </div>
              </div>,
              document.body
            )}
        </div>
      )}

      {dragGhostTab &&
        createPortal(
          <div ref={dragGhostRef} className="tab-drag-ghost" aria-hidden="true">
            {dragGhostTab.type === "settings" && <Settings className="tab-icon" size={13} />}
            {dragGhostTab.type === "tunnels" && <Waypoints className="tab-icon" size={13} />}
            {dragGhostTab.type === "logs" && <ScrollText className="tab-icon" size={13} />}
            <span className="tab-drag-ghost-title">{dragGhostTab.title}</span>
          </div>,
          document.body
        )}
    </div>
  )
}
