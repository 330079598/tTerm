import { useMemo, useRef, useState } from "react"

import type { RecentCommand } from "@/types/command"

/**
 * The bar's text, and ArrowUp / ArrowDown through the commands run lately,
 * newest first, as a shell's history goes. What was being typed comes back
 * past the newest one.
 */
export function useComposerHistory(recentCommands: RecentCommand[]) {
  const [text, setTextState] = useState("")
  // Where the recall has got to; -1 is the text being typed.
  const [index, setIndex] = useState(-1)
  const draftRef = useRef("")
  // The same command run on two hosts is one entry.
  const history = useMemo(
    () => [...new Set(recentCommands.map((command) => command.commandText))],
    [recentCommands]
  )

  const setText = (next: string) => {
    setTextState(next)
    setIndex(-1)
  }

  const recall = (next: number) => {
    if (index === -1) draftRef.current = text
    setIndex(next)
    setTextState(next === -1 ? draftRef.current : (history[next] ?? ""))
  }

  /** Moves to an older command; false when there is none. */
  const recallOlder = () => {
    if (index + 1 >= history.length) return false
    recall(index + 1)
    return true
  }

  /** Moves to a newer command or back to the draft; false when already there. */
  const recallNewer = () => {
    if (index < 0) return false
    recall(index - 1)
    return true
  }

  return { text, setText, recallOlder, recallNewer }
}
