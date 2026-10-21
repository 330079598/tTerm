import { useCallback, useRef, useState } from "react"

import { useConfig } from "@/contexts/ConfigContext"
import { useSettingsSave } from "@/hooks/useSettingsSave"
import { useStableRef } from "@/hooks/useStableRef"
import {
  type HighlightSet,
  highlightSets,
  type KeywordHighlightConfig,
} from "@/lib/keywordHighlight/config"
import { updateHighlightSet } from "@/lib/keywordHighlight/edit"

/**
 * The keyword highlight config as the settings page edits it. Each edit
 * builds on the previous one even before it is saved: settings are saved
 * whole, so an edit made from the config as last saved would undo it.
 */
export function useHighlightSettings() {
  const { config } = useConfig()
  const { saveSettings } = useSettingsSave()
  const [pending, setPending] = useState<KeywordHighlightConfig | null>(null)
  const pendingRef = useRef<KeywordHighlightConfig | null>(null)
  const savedRef = useStableRef(config.keyword_highlight)
  const highlight = pending ?? config.keyword_highlight

  const edit = useCallback(
    (change: (current: KeywordHighlightConfig) => KeywordHighlightConfig) => {
      const current = pendingRef.current ?? savedRef.current
      const next = change(current)
      if (next === current) return
      pendingRef.current = next
      setPending(next)
      void saveSettings({ keyword_highlight: next }).finally(() => {
        if (pendingRef.current !== next) return
        pendingRef.current = null
        setPending(null)
      })
    },
    [saveSettings, savedRef]
  )

  const editSet = useCallback(
    (setId: string, change: (set: HighlightSet) => HighlightSet) =>
      edit((current) => updateHighlightSet(current, setId, change)),
    [edit]
  )

  return { highlight, sets: highlightSets(highlight), edit, editSet }
}
