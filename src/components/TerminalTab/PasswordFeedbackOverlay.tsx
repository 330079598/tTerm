import type React from "react"
import { ArrowBigUpDash, KeyRound } from "lucide-react"
import { useTranslation } from "react-i18next"

import type { FeedbackLayout } from "@/components/TerminalTab/passwordPromptLayout"
import type { CursorCellView } from "@/components/TerminalTab/useCursorCellBox"

interface PasswordFeedbackOverlayProps {
  /** The cell after the cursor. */
  box: CursorCellView
  capsLockOn: boolean
  layout: FeedbackLayout
}

/**
 * Sits beside the cursor at a password prompt: a key, a Caps Lock warning in
 * its place when that is on, and a `*` per typed character the way sudo's
 * pwfeedback echoes them, or for a moment the words that Caps Lock is on.
 * tTerm draws it over the terminal; none of it is terminal content or
 * reaches the remote.
 */
export const PasswordFeedbackOverlay: React.FC<PasswordFeedbackOverlayProps> = ({
  box,
  capsLockOn,
  layout,
}) => {
  const { t } = useTranslation()
  const label = t(capsLockOn ? "sudoAutofill.tabWaitingCapsLock" : "sudoAutofill.cursorWaiting")
  const iconSize = Math.round(Math.min(box.cellHeight * 0.75, box.cellWidth * 1.6))
  const width = layout.cells * box.cellWidth

  return (
    <div
      className="terminal-password-feedback"
      data-caps-lock={capsLockOn || undefined}
      data-floating={layout.floating || undefined}
      role="status"
      aria-label={label}
      style={{
        left: layout.left,
        top: layout.top,
        height: box.cellHeight,
        ...(layout.floating
          ? { width, padding: `0 ${box.cellWidth / 2}px`, background: box.background }
          : { maxWidth: width }),
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
      {layout.text && (
        <span aria-hidden="true" data-notice={layout.notice || undefined}>
          {layout.text}
        </span>
      )}
    </div>
  )
}
