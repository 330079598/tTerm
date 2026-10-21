import { useCallback, useEffect, useMemo, useState } from "react"

import type { TerminalTabProps } from "@/components/TerminalTab/types"
import type { TerminalTabRefs } from "@/components/TerminalTab/useTerminalTabRefs"
import type { AppConfig } from "@/lib/appConfig"
import { resolveHighlightSet } from "@/lib/keywordHighlight/config"
import { compileHighlightSet, type HighlightPalette } from "@/lib/keywordHighlight/match"

type UseKeywordHighlightOptions = Pick<TerminalTabRefs, "keywordHighlighterRef"> &
  Pick<TerminalTabProps, "connection" | "isActive"> & {
    config: AppConfig
    /** The terminal's colors, which palette color names in rules take. */
    palette: HighlightPalette
  }

/**
 * Gives the terminal's keyword highlighter the rules of its connection's set,
 * in the terminal's colors, and lets the user pause it for this tab.
 */
export function useKeywordHighlight({
  keywordHighlighterRef,
  connection,
  isActive,
  config,
  palette,
}: UseKeywordHighlightOptions) {
  const [paused, setPaused] = useState(false)
  const set = useMemo(
    () => resolveHighlightSet(config.keyword_highlight, connection?.highlightSet),
    [config.keyword_highlight, connection?.highlightSet]
  )
  const rules = useMemo(
    () => (set && !paused ? compileHighlightSet(set, palette) : []),
    [palette, paused, set]
  )
  const alternateScreen = config.keyword_highlight.alternate_screen

  // Every commit: the terminal, and its highlighter with it, is created anew
  // for each session and on some setting changes. Unchanged values cost nothing.
  useEffect(() => {
    const highlighter = keywordHighlighterRef.current
    highlighter?.setRules(rules, alternateScreen)
    highlighter?.setVisible(isActive)
  })

  const toggleHighlightPaused = useCallback(() => setPaused((value) => !value), [])

  return {
    /** A set applies to this tab, so pausing it means something. */
    highlightAvailable: set !== null,
    highlightPaused: paused,
    toggleHighlightPaused,
  }
}
