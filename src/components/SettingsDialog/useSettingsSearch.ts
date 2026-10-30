import React, { useEffect, useMemo, useState } from "react"
import { useTranslation } from "react-i18next"

import {
  buildSettingsSearchItems,
  revealSetting,
  searchSettings,
  type SettingsSearchResult,
} from "@/components/SettingsDialog/settingsSearch"

/** How many frames to wait for a page to render the setting a result opens. */
const REVEAL_MAX_FRAMES = 90

interface UseSettingsSearchOptions {
  /** The settings panel; the search looks for pages only inside it. */
  rootRef: React.RefObject<HTMLElement | null>
  showTab: (tab: string) => void
}

/** Holds the settings search and opens a result on its page. */
export function useSettingsSearch({ rootRef, showTab }: UseSettingsSearchOptions) {
  const { t } = useTranslation()
  const [query, setQueryState] = useState("")
  const [activeIndex, setActiveIndex] = useState(0)
  const [pending, setPending] = useState<SettingsSearchResult | null>(null)

  // A language change hands out a new `t`, which rebuilds the items in it.
  const items = useMemo(() => buildSettingsSearchItems(t), [t])
  const results = useMemo(() => searchSettings(items, query), [items, query])

  const setQuery = (value: string) => {
    setQueryState(value)
    setActiveIndex(0)
  }

  const openResult = (result: SettingsSearchResult) => {
    setQuery("")
    showTab(result.tab)
    // A copy, so opening the same result again reveals it again.
    if (!result.isPage) setPending({ ...result })
  }

  // The page renders after the tab switch and may load its data first, so
  // the setting is looked for on each frame until it shows up.
  useEffect(() => {
    if (!pending) return undefined

    let frames = 0
    let handle = 0
    const tryReveal = () => {
      const panel = rootRef.current?.querySelector<HTMLElement>(`#settings-panel-${pending.tab}`)
      if (panel && revealSetting(panel, pending.title)) return
      frames += 1
      if (frames < REVEAL_MAX_FRAMES) handle = requestAnimationFrame(tryReveal)
    }
    handle = requestAnimationFrame(tryReveal)
    return () => cancelAnimationFrame(handle)
  }, [pending, rootRef])

  const handleKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Escape" && query) {
      // Clearing the search comes before closing the dialog.
      event.stopPropagation()
      setQuery("")
      return
    }
    if (results.length === 0) return

    if (event.key === "ArrowDown") {
      event.preventDefault()
      setActiveIndex((index) => (index + 1) % results.length)
    } else if (event.key === "ArrowUp") {
      event.preventDefault()
      setActiveIndex((index) => (index - 1 + results.length) % results.length)
    } else if (event.key === "Enter") {
      event.preventDefault()
      openResult(results[Math.min(activeIndex, results.length - 1)])
    }
  }

  return {
    query,
    setQuery,
    results,
    activeIndex,
    setActiveIndex,
    openResult,
    handleKeyDown,
  }
}

export type SettingsSearch = ReturnType<typeof useSettingsSearch>
