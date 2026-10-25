import type React from "react"
import { ArrowBigUpDash, KeyRound } from "lucide-react"
import { useTranslation } from "react-i18next"

import type { CursorCellView } from "@/components/TerminalTab/useCursorCellBox"
import { useConfig } from "@/contexts/ConfigContext"
import { useCapsLock } from "@/hooks/useCapsLock"

/** Cells the key icon and the gap after it take. */
const ICON_CELLS = 2

interface PasswordFeedbackOverlayProps {
  /** The cell after the cursor. */
  box: CursorCellView
  /** Characters typed at the prompt so far. */
  typed: number
}

/**
 * Sits beside the cursor at a password prompt: a key, a Caps Lock warning in
 * its place when that is on, and a `*` per typed character the way sudo's
 * pwfeedback echoes them. tTerm draws it over the terminal; none of it is
 * terminal content or reaches the remote.
 */
export const PasswordFeedbackOverlay: React.FC<PasswordFeedbackOverlayProps> = ({ box, typed }) => {
  const { t } = useTranslation()
  const { config } = useConfig()
  const capsLockOn = useCapsLock()

  const room = Math.max(0, box.cellsLeft - ICON_CELLS)
  const shownTyped = config.password_feedback ? typed : 0
  // A password longer than the rest of the row fills it, led by an ellipsis.
  const marks = shownTyped > room ? `…${"*".repeat(Math.max(0, room - 1))}` : "*".repeat(shownTyped)
  const label = t(capsLockOn ? "sudoAutofill.tabWaitingCapsLock" : "sudoAutofill.cursorWaiting")
  const iconSize = Math.round(Math.min(box.cellHeight * 0.75, box.cellWidth * 1.6))

  return (
    <div
      className="terminal-password-feedback"
      data-caps-lock={capsLockOn || undefined}
      role="status"
      aria-label={label}
      style={{
        left: box.left,
        top: box.top,
        height: box.cellHeight,
        maxWidth: Math.max(box.cellsLeft, ICON_CELLS) * box.cellWidth,
        color: box.color,
        fontFamily: box.fontFamily,
        fontSize: box.fontSize,
        lineHeight: `${box.cellHeight}px`,
      }}
    >
      {capsLockOn ? (
        <ArrowBigUpDash size={iconSize} aria-hidden="true" />
      ) : (
        <KeyRound size={iconSize} aria-hidden="true" />
      )}
      {marks && <span aria-hidden="true">{marks}</span>}
    </div>
  )
}
