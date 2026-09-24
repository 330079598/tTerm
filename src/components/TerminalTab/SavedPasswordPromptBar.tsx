import React from "react"
import { useTranslation } from "react-i18next"
import { KeyRound, ShieldAlert, X } from "lucide-react"

import { getConnectionDisplay } from "@/components/TerminalTab/terminalTabUtils"
import type { SavedPasswordPromptState, TerminalTabProps } from "@/components/TerminalTab/types"
import { formatChord, isMacPlatform } from "@/lib/keymap/chord"

interface SavedPasswordPromptBarProps {
  connection: TerminalTabProps["connection"]
  /** Serialized chord of the fill action, if bound. */
  fillShortcut?: string
  onDismiss: () => void
  onFill: () => void
  state: SavedPasswordPromptState
}

/**
 * Offers the saved password for a sudo prompt. It lives outside the terminal
 * canvas on purpose: remote output can imitate anything drawn in the terminal,
 * but not this bar, and filling needs its button or a dedicated shortcut.
 */
export const SavedPasswordPromptBar: React.FC<SavedPasswordPromptBarProps> = ({
  connection,
  fillShortcut,
  onDismiss,
  onFill,
  state,
}) => {
  const { t } = useTranslation()
  const keepTerminalFocus = (event: React.MouseEvent) => event.preventDefault()
  const title = state.user
    ? t("sudoAutofill.promptForUser", { user: state.user })
    : t("sudoAutofill.promptGeneric")

  return (
    <div
      className={`terminal-password-prompt is-${state.status}`}
      role={state.status === "rejected" ? "alert" : "status"}
      aria-live="polite"
    >
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
            : t(state.source === "sudo" ? "sudoAutofill.sourceSudo" : "sudoAutofill.sourceLogin", {
                profile: connection?.profileName ?? "",
                target: getConnectionDisplay(connection, t),
              })}
        </span>
      </div>
      {state.status === "available" && (
        <button type="button" className="primary" onMouseDown={keepTerminalFocus} onClick={onFill}>
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
  )
}
