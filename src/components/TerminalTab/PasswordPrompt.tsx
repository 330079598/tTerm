import React, { useMemo } from "react"
import type { Terminal } from "@xterm/xterm"

import { PasswordFeedbackOverlay } from "@/components/TerminalTab/PasswordFeedbackOverlay"
import { promptInputRect } from "@/components/TerminalTab/passwordPromptLayout"
import { SavedPasswordPromptBar } from "@/components/TerminalTab/SavedPasswordPromptBar"
import type { SavedPasswordPromptState, TerminalTabProps } from "@/components/TerminalTab/types"
import { useCursorCellBox } from "@/components/TerminalTab/useCursorCellBox"

interface PasswordPromptProps {
  termRef: React.RefObject<Terminal | null>
  surfaceRef: React.RefObject<HTMLDivElement | null>
  connection: TerminalTabProps["connection"]
  /** Serialized chord of the fill action, if bound. */
  fillShortcut?: string
  onDismiss: () => void
  onFill: () => void
  state: SavedPasswordPromptState
}

/**
 * Everything tTerm shows while a password prompt waits: the feedback beside
 * the cursor and, when a saved password is on offer, the bar. Both follow the
 * same measured cursor, so the bar can keep clear of the prompt line.
 */
export const PasswordPrompt: React.FC<PasswordPromptProps> = ({
  termRef,
  surfaceRef,
  state,
  ...barProps
}) => {
  const box = useCursorCellBox(termRef, surfaceRef)
  const input = useMemo(() => box && promptInputRect(box), [box])

  return (
    <>
      {box && (
        <PasswordFeedbackOverlay box={box} typed={state.status === "manual" ? state.typed : 0} />
      )}
      {state.status !== "manual" && (
        <SavedPasswordPromptBar {...barProps} input={input} state={state} />
      )}
    </>
  )
}
