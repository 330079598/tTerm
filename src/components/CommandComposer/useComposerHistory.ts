import { useMemo, useRef, useState } from "react"

import { setComposerText } from "@/lib/commandComposer"
import type { RecentCommand } from "@/types/command"

/**
 * ArrowUp / ArrowDown through the commands run lately, newest first, as a
 * shell's history goes, in the bar's text. What was being typed comes back
 * past the newest one.
 */
export function useComposerHistory(text: string, recentCommands: RecentCommand[]) {
  // Where the recall has got to; -1 is the text being typed.
  const [index, setIndex] = useState(-1)
  const draftRef = useRef("")
  // The same command run on two hosts is one entry.
  const history = useMemo(
    () => [...new Set(recentCommands.map((command) => command.commandText))],
    [recentCommands]
  )

  const setText = (next: string) => {
    setComposerText(next)
    setIndex(-1)
  }

  const recall = (next: number) => {
    if (index === -1) draftRef.current = text
    setIndex(next)
    setComposerText(next === -1 ? draftRef.current : (history[next] ?? ""))
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

  return { history, setText, recallOlder, recallNewer }
}
