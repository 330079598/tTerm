import { useCallback } from "react"

import type { AppShell } from "@/components/TTermApp/appShell"
import {
  useCollapsedTabsStorage,
  type CollapsedContent,
} from "@/components/TTermApp/useCollapsedTabsStorage"
import {
  captureTerminalContent,
  showContentWhenOpened,
  type SavedTerminalContent,
} from "@/components/TerminalTab/savedTerminalContent"
import type { ClosedTab } from "@/lib/closedTabs"
import {
  addCollapsedSet,
  collapsedGroupLayout,
  collapsedSetOf,
  collapsibleGroupIds,
  removeCollapsedGroup,
  removeCollapsedSet,
  type CollapsedGroup,
  type CollapsedSet,
} from "@/lib/collapsedTabs"
import { findGroupOfTab, getPaneIds } from "@/lib/tabGroups"
import type { Tab } from "@/types/tab"

interface CollapsedTabsDeps {
  /** Closes tabs after the usual confirmations, without keeping them for undo. */
  closeTabsForCollapse: (targetTabs: Tab[], closeAction: () => void) => Promise<boolean>
  /** Makes a closed tab ready to open again. */
  prepareTab: (tab: ClosedTab) => Promise<ClosedTab>
  /** Whether an AI agent session a tab ran resumes when the tab opens again. */
  resumeAgentSessions: boolean
}

function createId(): string {
  return crypto.randomUUID()
}

/** The content keys of `groups`, those that name any. */
function contentKeysOf(groups: readonly CollapsedGroup[]): string[] {
  return groups.flatMap((group) => group.contentKeys.flatMap((key) => key ?? []))
}

/** Hands each reopened tab, by its new id, the content kept for it. */
function showKeptContent(
  group: CollapsedGroup,
  ids: readonly string[],
  content: ReadonlyMap<string, SavedTerminalContent>
) {
  group.contentKeys.forEach((key, index) => {
    const saved = key ? content.get(key) : undefined
    if (saved && ids[index]) showContentWhenOpened(ids[index], saved)
  })
}

/**
 * The tabs put away from the bar, OneTab style. Collapsing closes their
 * sessions and keeps what reopening them needs, with what their terminals
 * showed, across launches. Opening one again connects it anew in its split,
 * at the end of the bar, with that content above the new session, as a
 * restarted tTerm shows its tabs.
 */
export function useCollapsedTabs(shell: AppShell, deps: CollapsedTabsDeps) {
  const { t, tabsRef, groups, removeTabs, reopenTabs, reopenTabGroups } = shell
  const { closeTabsForCollapse, prepareTab, resumeAgentSessions } = deps
  const { sets, setSets, saveContent, takeContent } = useCollapsedTabsStorage(t)

  const collapseGroups = useCallback(
    async (groupIds: string[]) => {
      const kept: CollapsedContent[] = []
      // Read before the tabs close, which disposes their terminals.
      const keepContent = (tabId: string) => {
        const content = captureTerminalContent(tabId)
        if (!content) return null
        const key = `collapsed-${createId()}`
        kept.push({ tabId: key, content })
        return key
      }
      const set = collapsedSetOf(tabsRef.current, groups, groupIds, {
        now: Date.now(),
        createId,
        keepContent,
      })
      if (!set) return
      const wanted = new Set(groupIds)
      const ids = groups
        .filter((group) => wanted.has(group.id))
        .flatMap((group) => getPaneIds(group.root))
      const idSet = new Set(ids)
      const targetTabs = tabsRef.current.filter((tab) => idSet.has(tab.id))
      if (!(await closeTabsForCollapse(targetTabs, () => removeTabs(ids)))) return
      saveContent(kept)
      setSets((current) => addCollapsedSet(current, set))
    },
    [closeTabsForCollapse, groups, removeTabs, saveContent, setSets, tabsRef]
  )

  /** Puts away every tab of the bar that holds a session. */
  const collapseAll = useCallback(
    () => collapseGroups(collapsibleGroupIds(tabsRef.current, groups)),
    [collapseGroups, groups, tabsRef]
  )

  /** Puts away the tab of the bar holding pane `id`, with all its panes. */
  const collapseTab = useCallback(
    (id: string) => {
      const group = findGroupOfTab(groups, id)
      if (group) void collapseGroups([group.id])
    },
    [collapseGroups, groups]
  )

  const prepareGroup = useCallback(
    async (group: CollapsedGroup) => {
      // As a restart does, an agent session resumes only when that is on.
      const tabs = group.tabs.map((tab) =>
        resumeAgentSessions || !tab.connection?.agentResume
          ? tab
          : { ...tab, connection: { ...tab.connection, agentResume: undefined } }
      )
      return { tabs: await Promise.all(tabs.map(prepareTab)), layout: collapsedGroupLayout(group) }
    },
    [prepareTab, resumeAgentSessions]
  )

  /** Opens one tab of the bar of a set again, and takes it out of the set. */
  const restoreGroup = useCallback(
    async (setId: string, group: CollapsedGroup) => {
      // Read before the set changes, as saving it deletes content it no longer names.
      const content = takeContent(contentKeysOf([group]))
      setSets((current) => removeCollapsedGroup(current, setId, group.id))
      const [{ tabs, layout }, kept] = await Promise.all([prepareGroup(group), content])
      showKeptContent(group, reopenTabs(tabs, layout), kept)
    },
    [prepareGroup, reopenTabs, setSets, takeContent]
  )

  /** Opens every tab of the bar of a set again; only the first connects right away. */
  const restoreSet = useCallback(
    async (set: CollapsedSet) => {
      const content = takeContent(contentKeysOf(set.groups))
      setSets((current) => removeCollapsedSet(current, set.id))
      const [entries, kept] = await Promise.all([
        Promise.all(set.groups.map(prepareGroup)),
        content,
      ])
      const ids = reopenTabGroups(entries)
      set.groups.forEach((group, index) => showKeptContent(group, ids[index] ?? [], kept))
    },
    [prepareGroup, reopenTabGroups, setSets, takeContent]
  )

  const deleteGroup = useCallback(
    (setId: string, groupId: string) => {
      setSets((current) => removeCollapsedGroup(current, setId, groupId))
    },
    [setSets]
  )

  const deleteSet = useCallback(
    (setId: string) => {
      setSets((current) => removeCollapsedSet(current, setId))
    },
    [setSets]
  )

  return {
    sets,
    canCollapse: collapsibleGroupIds(shell.tabs, groups).length > 0,
    collapseAll,
    collapseTab,
    restoreGroup,
    restoreSet,
    deleteGroup,
    deleteSet,
  }
}

export type CollapsedTabs = ReturnType<typeof useCollapsedTabs>
