import { useState, useCallback, useRef } from "react"
import { Tab } from "@/types/tab"

export interface UseTabsReturn {
  tabs: Tab[]
  activeTabId: string | null
  addTab: (tab: Omit<Tab, "id" | "isActive">) => string
  openSettingsTab: (title: string) => void
  renameSettingsTab: (title: string) => void
  removeTab: (id: string) => void
  removeTabs: (ids: string[]) => void
  setActiveTab: (id: string) => void
  moveTab: (fromIndex: number, toIndex: number) => void
  duplicateTab: (id: string) => string | null
  closeOtherTabs: (id: string) => void
  closeTabsToRight: (id: string) => void
  closeTabsToLeft: (id: string) => void
  renameTab: (id: string, newTitle: string) => void
  restoreSession: (tabs: Tab[], activeTabId: string | null) => void
  updateTab: (id: string, updater: (tab: Tab) => Tab) => void
}

function ensureTabDefaults(tab: Tab): Tab {
  if (tab.type === "settings" || tab.type === "remote-file-editor") {
    return {
      ...tab,
      hasConnected: true,
      sessionNonce: 0,
      connection: tab.type === "settings" ? undefined : tab.connection,
    }
  }

  return {
    ...tab,
    hasConnected: tab.hasConnected ?? tab.isActive,
    sessionNonce: tab.sessionNonce ?? 0,
    connectionHeaderPinned: tab.connectionHeaderPinned ?? true,
  }
}

function nextSessionNonce(sessionNonce?: number) {
  return ((sessionNonce ?? 0) + 1) >>> 0
}

export function useTabs(): UseTabsReturn {
  const [tabs, setTabs] = useState<Tab[]>([])
  const [activeTabId, setActiveTabId] = useState<string | null>(null)
  const tabIdCounter = useRef(0)

  const generateTabId = useCallback(() => {
    return `tab-${++tabIdCounter.current}`
  }, [])

  // Restore session
  const restoreSession = useCallback((restoredTabs: Tab[], restoredActiveTabId: string | null) => {
    // Update counter to avoid ID conflicts
    const maxId = restoredTabs.reduce((max, tab) => {
      const match = tab.id.match(/tab-(\d+)/)
      if (match) {
        const num = parseInt(match[1], 10)
        return Math.max(max, num)
      }
      return max
    }, 0)
    tabIdCounter.current = maxId

    setTabs(
      restoredTabs.map((tab) =>
        ensureTabDefaults({
          ...tab,
          hasConnected: tab.id === restoredActiveTabId,
        })
      )
    )
    setActiveTabId(restoredActiveTabId)
  }, [])

  const addTab = useCallback(
    (tabData: Omit<Tab, "id" | "isActive">) => {
      const id = generateTabId()
      const newTab: Tab = ensureTabDefaults({
        ...tabData,
        id,
        isActive: false,
        hasConnected: true,
      })

      setTabs((prevTabs) => {
        const updatedTabs = prevTabs.map((tab) => ({ ...tab, isActive: false }))
        return [...updatedTabs, { ...newTab, isActive: true, hasConnected: true }]
      })

      setActiveTabId(id)
      return id
    },
    [generateTabId]
  )

  const openSettingsTab = useCallback(
    (title: string) => {
      setTabs((prevTabs) => {
        const existingSettingsTab = prevTabs.find((tab) => tab.type === "settings")
        if (existingSettingsTab) {
          setActiveTabId(existingSettingsTab.id)
          return prevTabs.map((tab) => ({
            ...tab,
            isActive: tab.id === existingSettingsTab.id,
            hasConnected: tab.id === existingSettingsTab.id ? true : tab.hasConnected,
          }))
        }

        const id = generateTabId()
        const newTab: Tab = ensureTabDefaults({
          id,
          title,
          type: "settings",
          isActive: true,
          isModified: false,
          hasConnected: true,
        })

        setActiveTabId(id)
        const updatedTabs = prevTabs.map((tab) => ({ ...tab, isActive: false }))
        return [...updatedTabs, newTab]
      })
    },
    [generateTabId]
  )

  const renameSettingsTab = useCallback((title: string) => {
    setTabs((prevTabs) => {
      let hasChanges = false
      const updatedTabs = prevTabs.map((tab) => {
        if (tab.type !== "settings" || tab.title === title) {
          return tab
        }

        hasChanges = true
        return { ...tab, title }
      })

      return hasChanges ? updatedTabs : prevTabs
    })
  }, [])

  const removeTab = useCallback(
    (id: string) => {
      setTabs((prevTabs) => {
        const tabIndex = prevTabs.findIndex((tab) => tab.id === id)
        if (tabIndex === -1) return prevTabs

        const newTabs = prevTabs.filter((tab) => tab.id !== id)

        // If we're removing the active tab, activate another one
        if (activeTabId === id && newTabs.length > 0) {
          let newActiveIndex = tabIndex
          if (newActiveIndex >= newTabs.length) {
            newActiveIndex = newTabs.length - 1
          }
          const newActiveTabId = newTabs[newActiveIndex].id
          setActiveTabId(newActiveTabId)
          return newTabs.map((t) => ({
            ...t,
            isActive: t.id === newActiveTabId,
            hasConnected: t.id === newActiveTabId ? true : t.hasConnected,
          }))
        } else if (newTabs.length === 0) {
          setActiveTabId(null)
        }

        return newTabs
      })
    },
    [activeTabId]
  )

  const removeTabs = useCallback(
    (ids: string[]) => {
      const idsToRemove = new Set(ids)
      if (idsToRemove.size === 0) return

      setTabs((prevTabs) => {
        const activeTabIndex = prevTabs.findIndex((tab) => tab.id === activeTabId)
        const remainingTabs = prevTabs.filter((tab) => !idsToRemove.has(tab.id))

        if (!idsToRemove.has(activeTabId ?? "")) {
          return remainingTabs
        }

        if (remainingTabs.length === 0) {
          setActiveTabId(null)
          return remainingTabs
        }

        const newActiveTab = remainingTabs[Math.min(activeTabIndex, remainingTabs.length - 1)]
        setActiveTabId(newActiveTab.id)
        return remainingTabs.map((tab) => ({
          ...tab,
          isActive: tab.id === newActiveTab.id,
          hasConnected: tab.id === newActiveTab.id ? true : tab.hasConnected,
        }))
      })
    },
    [activeTabId]
  )

  const setActiveTab = useCallback((id: string) => {
    setTabs((prevTabs) =>
      prevTabs.map((tab) => ({
        ...tab,
        isActive: tab.id === id,
        hasConnected: tab.id === id ? true : tab.hasConnected,
      }))
    )
    setActiveTabId(id)
  }, [])

  const moveTab = useCallback((fromIndex: number, toIndex: number) => {
    setTabs((prevTabs) => {
      const newTabs = [...prevTabs]
      const [movedTab] = newTabs.splice(fromIndex, 1)
      newTabs.splice(toIndex, 0, movedTab)
      return newTabs
    })
  }, [])

  const duplicateTab = useCallback(
    (id: string) => {
      const tab = tabs.find((candidate) => candidate.id === id)
      if (!tab || tab.type === "settings" || tab.type === "remote-file-editor") {
        return null
      }

      const newId = generateTabId()
      const { id: _id, isActive: _isActive, ...tabData } = tab
      const newTab: Tab = ensureTabDefaults({
        ...tabData,
        connection: tabData.connection ?? {
          type: tabData.type === "terminal" ? "terminal" : "ssh",
        },
        sessionNonce: nextSessionNonce(tab.sessionNonce),
        id: newId,
        title: `${tab.title} (Copy)`,
        isActive: true,
        hasConnected: true,
      })

      setTabs((prevTabs) => [
        ...prevTabs.map((currentTab) => ({ ...currentTab, isActive: false })),
        newTab,
      ])
      setActiveTabId(newId)
      return newId
    },
    [generateTabId, tabs]
  )

  const closeOtherTabs = useCallback((id: string) => {
    setTabs((prevTabs) => {
      const tabToKeep = prevTabs.find((tab) => tab.id === id)
      if (!tabToKeep) return prevTabs

      setActiveTabId(id)
      return [{ ...tabToKeep, isActive: true, hasConnected: true }]
    })
  }, [])

  const closeTabsToRight = useCallback((id: string) => {
    setTabs((prevTabs) => {
      const tabIndex = prevTabs.findIndex((tab) => tab.id === id)
      if (tabIndex === -1) return prevTabs

      return prevTabs.slice(0, tabIndex + 1)
    })
  }, [])

  const closeTabsToLeft = useCallback((id: string) => {
    setTabs((prevTabs) => {
      const tabIndex = prevTabs.findIndex((tab) => tab.id === id)
      if (tabIndex <= 0) return prevTabs

      return prevTabs.slice(tabIndex)
    })
  }, [])

  const renameTab = useCallback((id: string, newTitle: string) => {
    setTabs((prevTabs) =>
      prevTabs.map((tab) => (tab.id === id ? { ...tab, title: newTitle } : tab))
    )
  }, [])

  const updateTab = useCallback((id: string, updater: (tab: Tab) => Tab) => {
    setTabs((prevTabs) =>
      prevTabs.map((tab) => (tab.id === id ? ensureTabDefaults(updater(tab)) : tab))
    )
  }, [])

  return {
    tabs,
    activeTabId,
    addTab,
    openSettingsTab,
    renameSettingsTab,
    removeTab,
    removeTabs,
    setActiveTab,
    moveTab,
    duplicateTab,
    closeOtherTabs,
    closeTabsToRight,
    closeTabsToLeft,
    renameTab,
    restoreSession,
    updateTab,
  }
}
