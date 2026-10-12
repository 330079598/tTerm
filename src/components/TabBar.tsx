import "@/components/TabBar.css"
import React, { useCallback, useEffect, useRef, useState } from "react"
import { createPortal } from "react-dom"
import { useTranslation } from "react-i18next"
import { ChevronDown, ChevronLeft, ChevronRight } from "lucide-react"

import { useTabDrag } from "@/components/TabBar/useTabDrag"
import { useTabDragState } from "@/components/TabBar/useTabDragState"
import { useTabOverflowMenu } from "@/components/TabBar/useTabOverflowMenu"
import { TabOverflowPanel } from "@/components/TabBar/TabOverflowPanel"
import { TabItem } from "@/components/TabBar/TabItem"
import { TabIcon } from "@/components/TabIcon"
import type { Tab, TabContextMenuAction } from "@/types/tab"
import { focusActiveTerminal, keepFocusOnMouseDown } from "@/lib/terminalFocus"

const TAB_OVERFLOW_THRESHOLD = 16
export interface TabBarProps {
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
  const {
    dragStateRef,
    dragGhostRef,
    lastPointerPosRef,
    autoScrollRef,
    pendingFlipRef,
    tabsRef,
    suppressNextClickRef,
    draggingTabId,
    setDraggingTabId,
    dragMode,
    setDragMode,
    dragGhostTab,
    setDragGhostTab,
  } = useTabDragState(tabs)
  const [scrollState, setScrollState] = useState({ canScrollLeft: false, canScrollRight: false })
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

  const {
    overflowMenuRef,
    overflowTriggerRef,
    overflowPanelRef,
    searchInputRef,
    isOverflowMenuOpen,
    setIsOverflowMenuOpen,
    searchQuery,
    setSearchQuery,
    overflowPanelStyle,
    updateOverflowPanelPosition,
    closeOverflowMenu,
    handleSelectTab,
    filteredTabs,
    handleSearchKeyDown,
  } = useTabOverflowMenu({ tabs, onTabClick })

  const { layoutListDrag, handleTabPointerDown } = useTabDrag({
    dragStateRef,
    dragGhostRef,
    lastPointerPosRef,
    autoScrollRef,
    pendingFlipRef,
    tabsRef,
    suppressNextClickRef,
    setDraggingTabId,
    setDragMode,
    setDragGhostTab,
    onTabDragMove,
    onTabDrop,
    onTabDragCancel,
    onTabMove,
    listRef,
    viewportRef,
  })

  const handleTabListScroll = useCallback(() => {
    updateScrollState()
    const dragState = dragStateRef.current
    const pointer = lastPointerPosRef.current
    if (dragState?.dragging && dragState.mode === "list" && pointer) {
      layoutListDrag(dragState, pointer.x)
    }
  }, [layoutListDrag, updateScrollState, dragStateRef, lastPointerPosRef])

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
    [activeTabId, onTabClick, suppressNextClickRef]
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
              <TabOverflowPanel
                overflowPanelRef={overflowPanelRef}
                overflowPanelStyle={overflowPanelStyle}
                searchInputRef={searchInputRef}
                searchQuery={searchQuery}
                setSearchQuery={setSearchQuery}
                handleSearchKeyDown={handleSearchKeyDown}
                filteredTabs={filteredTabs}
                tabs={tabs}
                activeTabId={activeTabId}
                handleSelectTab={handleSelectTab}
              />,
              document.body
            )}
        </div>
      )}

      {dragGhostTab &&
        createPortal(
          <div ref={dragGhostRef} className="tab-drag-ghost" aria-hidden="true">
            <TabIcon tab={dragGhostTab} className="tab-icon" />
            <span className="tab-drag-ghost-title">{dragGhostTab.title}</span>
          </div>,
          document.body
        )}
    </div>
  )
}
