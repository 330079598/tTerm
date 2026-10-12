import React, { useCallback, useEffect, useRef, useState } from "react"

import type { TabBarProps } from "@/components/TabBar"
import { placeOverflowPanel, type OverflowPanelStyle } from "@/components/TabBar/overflowPanelStyle"
import { isImeKeyEvent } from "@/lib/ime"
import { refocusTerminalIfFocusLost } from "@/lib/terminalFocus"

/**
 * The menu listing every tab when they overflow the bar: where its panel
 * opens, searching it, and closing it on a click outside or Escape.
 */
export function useTabOverflowMenu({ tabs, onTabClick }: Pick<TabBarProps, "tabs" | "onTabClick">) {
  const overflowMenuRef = useRef<HTMLDivElement | null>(null)
  const overflowTriggerRef = useRef<HTMLButtonElement | null>(null)
  const overflowPanelRef = useRef<HTMLDivElement | null>(null)
  const searchInputRef = useRef<HTMLInputElement | null>(null)
  const [isOverflowMenuOpen, setIsOverflowMenuOpen] = useState(false)
  const [searchQuery, setSearchQuery] = useState("")
  const [overflowPanelStyle, setOverflowPanelStyle] = useState<OverflowPanelStyle | null>(null)

  const updateOverflowPanelPosition = useCallback(() => {
    const trigger = overflowTriggerRef.current
    if (!trigger) {
      return
    }

    setOverflowPanelStyle(
      placeOverflowPanel(trigger.getBoundingClientRect(), window.innerWidth, window.innerHeight)
    )
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

  const handleSelectTab = useCallback(
    (id: string) => {
      onTabClick(id)
      closeOverflowMenu()
    },
    [closeOverflowMenu, onTabClick]
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
      if (event.key === "Enter" && !isImeKeyEvent(event.nativeEvent) && filteredTabs.length > 0) {
        handleSelectTab(filteredTabs[0].id)
      }
    },
    [filteredTabs, handleSelectTab]
  )

  return {
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
  }
}
