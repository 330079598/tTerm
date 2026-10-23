import { useCallback, useEffect, useMemo, useState } from "react"

import type { LayoutTemplateTarget } from "@/components/LayoutTemplateDialog"
import type { AppShell } from "@/components/TTermApp/appShell"
import { useKeymap } from "@/contexts/KeymapContext"
import { findGroupOfTab } from "@/lib/tabGroups"
import { isSessionTab } from "@/types/tab"

/**
 * The layout template dialog: which pane it lays out, opened from a tab's
 * menu or by its shortcut for the active pane.
 */
export function useLayoutTemplates(shell: AppShell) {
  const { tabs, groups, activeTabId, applyLayoutTemplate } = shell
  const { registerHandler } = useKeymap()
  const [tabId, setTabId] = useState<string | null>(null)

  const openLayoutTemplates = useCallback((id: string) => setTabId(id), [])
  const closeLayoutTemplates = useCallback(() => setTabId(null), [])

  useEffect(
    () =>
      registerHandler("workspace.chooseLayout", () => {
        if (activeTabId) setTabId(activeTabId)
      }),
    [activeTabId, registerHandler]
  )

  // The target follows the tab as it is, and the dialog closes when the tab does.
  const target = useMemo<LayoutTemplateTarget | null>(() => {
    const tab = tabId ? tabs.find((candidate) => candidate.id === tabId) : undefined
    const group = tabId ? findGroupOfTab(groups, tabId) : undefined
    if (!tab || !group) return null
    return { tabId: tab.id, root: group.root, canCopy: isSessionTab(tab) }
  }, [groups, tabId, tabs])

  return {
    openLayoutTemplates,
    layoutTemplateDialog: {
      target,
      onApply: applyLayoutTemplate,
      onClose: closeLayoutTemplates,
    },
  }
}
