import React, { useMemo } from "react"
import type { Terminal } from "@xterm/xterm"

import { PasswordFeedbackOverlay } from "@/components/TerminalTab/PasswordFeedbackOverlay"
import { feedbackLayout, promptInputRect } from "@/components/TerminalTab/passwordPromptLayout"
import { SavedPasswordPromptBar } from "@/components/TerminalTab/SavedPasswordPromptBar"
import type { SavedPasswordPromptState, TerminalTabProps } from "@/components/TerminalTab/types"
import { useCursorCellBox } from "@/components/TerminalTab/useCursorCellBox"
import { useConfig } from "@/contexts/ConfigContext"

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
  const { config } = useConfig()
  const box = useCursorCellBox(termRef, surfaceRef)
  const typed = config.password_feedback ? (state.status === "manual" ? state.typed : 0) : null
  const layout = useMemo(() => box && feedbackLayout(box, typed), [box, typed])
  const input = useMemo(() => box && layout && promptInputRect(box, layout), [box, layout])

  return (
    <>
      {box && layout && <PasswordFeedbackOverlay box={box} layout={layout} />}
      {state.status !== "manual" && (
        <SavedPasswordPromptBar {...barProps} input={input} state={state} />
      )}
    </>
  )
}
