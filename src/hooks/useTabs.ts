import { useState, useCallback, useRef } from "react"
import { Tab } from "@/types/tab"

export interface UseTabsReturn {
  tabs: Tab[]
  activeTabId: string | null
  addTab: (tab: Omit<Tab, "id" | "isActive">) => string
  removeTab: (id: string) => void
  setActiveTab: (id: string) => void
  moveTab: (fromIndex: number, toIndex: number) => void
  duplicateTab: (id: string) => void
  closeOtherTabs: (id: string) => void
  closeTabsToRight: (id: string) => void
  renameTab: (id: string, newTitle: string) => void
  restoreSession: (tabs: Tab[], activeTabId: string | null) => void
  updateTab: (id: string, updater: (tab: Tab) => Tab) => void
}

function ensureTabDefaults(tab: Tab): Tab {
  return {
    ...tab,
    hasConnected: tab.hasConnected ?? tab.isActive,
    sessionNonce: tab.sessionNonce ?? 0,
    connectionHeaderPinned: tab.connectionHeaderPinned ?? true,
  }
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
      setTabs((prevTabs) => {
        const tab = prevTabs.find((t) => t.id === id)
        if (!tab) return prevTabs

        const newId = generateTabId()
        const { id: _id, isActive: _isActive, ...tabData } = tab
        const newTab: Tab = ensureTabDefaults({
          ...tabData,
          connection: tabData.connection ?? {
            type: tabData.type === "terminal" ? "terminal" : "ssh",
          },
          sessionNonce: (tab.sessionNonce ?? 0) + 1,
          id: newId,
          title: `${tab.title} (Copy)`,
          isActive: false,
          hasConnected: true,
        })

        // Set all tabs to inactive, new tab to active
        const updatedTabs = prevTabs.map((t) => ({ ...t, isActive: false }))
        setActiveTabId(newId)

        return [...updatedTabs, { ...newTab, isActive: true, hasConnected: true }]
      })
    },
    [generateTabId]
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
    removeTab,
    setActiveTab,
    moveTab,
    duplicateTab,
    closeOtherTabs,
    closeTabsToRight,
    renameTab,
    restoreSession,
    updateTab,
  }
}
