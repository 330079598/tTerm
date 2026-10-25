import React, { useRef } from "react"
import { useTranslation } from "react-i18next"
import { KeyRound, ShieldAlert, X } from "lucide-react"

import type { SurfaceRect } from "@/components/TerminalTab/passwordPromptLayout"
import { getConnectionDisplay } from "@/components/TerminalTab/terminalTabUtils"
import type { SavedPasswordPromptState, TerminalTabProps } from "@/components/TerminalTab/types"
import { usePromptBarPlacement } from "@/components/TerminalTab/usePromptBarPlacement"
import { formatChord, isMacPlatform } from "@/lib/keymap/chord"

interface SavedPasswordPromptBarProps {
  connection: TerminalTabProps["connection"]
  /** Serialized chord of the fill action, if bound. */
  fillShortcut?: string
  /** The prompt line, which the bar keeps clear of. */
  input: SurfaceRect | null
  onDismiss: () => void
  onFill: () => void
  /** The cursor overlay, not the bar, shows a prompt the user types into. */
  state: Exclude<SavedPasswordPromptState, { status: "manual" }>
}

/**
 * Offers the saved password for a sudo prompt. It lives outside the terminal
 * canvas on purpose: remote output can imitate anything drawn in the terminal,
 * but not this bar, and filling needs its button or a dedicated shortcut.
 */
export const SavedPasswordPromptBar: React.FC<SavedPasswordPromptBarProps> = ({
  connection,
  fillShortcut,
  input,
  onDismiss,
  onFill,
  state,
}) => {
  const { t } = useTranslation()
  const barRef = useRef<HTMLDivElement>(null)
  const placement = usePromptBarPlacement(barRef, input)
  const keepTerminalFocus = (event: React.MouseEvent) => event.preventDefault()
  const title = state.user
    ? t("sudoAutofill.promptForUser", { user: state.user })
    : t("sudoAutofill.promptGeneric")

  return (
    <div
      ref={barRef}
      className={`terminal-password-prompt is-${state.status} is-${placement}`}
      role={state.status === "rejected" ? "alert" : "status"}
      aria-live="polite"
    >
      <div className="terminal-password-prompt-main">
        {state.status === "rejected" ? (
          <ShieldAlert size={16} aria-hidden="true" />
        ) : (
          <KeyRound size={16} aria-hidden="true" />
        )}
        <div className="terminal-password-prompt-text">
          <strong>{state.status === "rejected" ? t("sudoAutofill.rejectedTitle") : title}</strong>
          <span>
            {state.status === "rejected"
              ? t("sudoAutofill.rejectedDescription")
              : t(
                  state.source === "sudo" ? "sudoAutofill.sourceSudo" : "sudoAutofill.sourceLogin",
                  {
                    profile: connection?.profileName ?? "",
                    target: getConnectionDisplay(connection, t),
                  }
                )}
          </span>
        </div>
      </div>
      <div className="terminal-password-prompt-actions">
        {state.status === "available" && (
          <button
            type="button"
            className="primary"
            onMouseDown={keepTerminalFocus}
            onClick={onFill}
          >
            {t("sudoAutofill.fill")}
            {fillShortcut && <kbd>{formatChord(fillShortcut, isMacPlatform())}</kbd>}
          </button>
        )}
        <button
          type="button"
          className="icon"
          aria-label={t("sudoAutofill.dismiss")}
          title={t("sudoAutofill.dismiss")}
          onMouseDown={keepTerminalFocus}
          onClick={onDismiss}
        >
          <X size={14} aria-hidden="true" />
        </button>
      </div>
    </div>
  )
}
