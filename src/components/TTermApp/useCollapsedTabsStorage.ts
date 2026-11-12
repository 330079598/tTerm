import { useCallback, useEffect, useRef, useState } from "react"
import type { TFunction } from "i18next"

import type { SavedTerminalContent } from "@/components/TerminalTab/savedTerminalContent"
import { parseCollapsedSets, type CollapsedSet } from "@/lib/collapsedTabs"
import { invokeSafe } from "@/lib/errors"

/** A terminal's content to keep for a collapsed tab, under its key. */
export interface CollapsedContent {
  tabId: string
  content: string
}

/**
 * The collapsed tabs as stored: read once, written whenever they change,
 * and the terminal content kept for them. Every call to the backend runs
 * after the one before, so saving the sets, which deletes the content they
 * no longer name, never runs ahead of the content being written or read.
 */
export function useCollapsedTabsStorage(t: TFunction) {
  const [sets, setSets] = useState<CollapsedSet[]>([])
  // Nothing is saved before the stored sets are read, or they would be lost.
  const loadedRef = useRef(false)
  const queueRef = useRef<Promise<unknown>>(Promise.resolve())

  const enqueue = useCallback(<T>(task: () => Promise<T>): Promise<T> => {
    const run = queueRef.current.then(task)
    queueRef.current = run.catch(() => undefined)
    return run
  }, [])

  useEffect(() => {
    let cancelled = false
    void enqueue(() =>
      invokeSafe<unknown>("load_collapsed_tabs", undefined, {
        title: t("collapsedTabs.loadFailed"),
      })
    ).then((result) => {
      // When they could not be read, saving would overwrite them, so tabs
      // collapsed from now on are kept for this run only.
      if (cancelled || !result.ok) return
      loadedRef.current = true
      // A set collapsed while they loaded comes first.
      setSets((current) => [...current, ...parseCollapsedSets(result.value)])
    })
    return () => {
      cancelled = true
    }
  }, [enqueue, t])

  useEffect(() => {
    if (!loadedRef.current) return
    void enqueue(() =>
      invokeSafe<void>("save_collapsed_tabs", { sets }, { title: t("collapsedTabs.saveFailed") })
    )
  }, [enqueue, sets, t])

  /** Keeps terminal content for tabs being collapsed. */
  const saveContent = useCallback(
    (snapshots: CollapsedContent[]) => {
      if (snapshots.length === 0) return
      void enqueue(() =>
        invokeSafe<void>(
          "save_collapsed_content",
          { snapshots },
          { title: t("collapsedTabs.saveFailed") }
        )
      )
    },
    [enqueue, t]
  )

  /** Reads, and lets go of, the content kept under `keys`, by key. */
  const takeContent = useCallback(
    async (keys: string[]): Promise<Map<string, SavedTerminalContent>> => {
      if (keys.length === 0) return new Map()
      const result = await enqueue(() =>
        invokeSafe<SavedTerminalContent[]>(
          "take_collapsed_content",
          { keys },
          { title: t("collapsedTabs.loadFailed"), silent: true }
        )
      )
      return new Map(result.ok ? result.value.map((saved) => [saved.tabId, saved]) : [])
    },
    [enqueue, t]
  )

  return { sets, setSets, saveContent, takeContent }
}
