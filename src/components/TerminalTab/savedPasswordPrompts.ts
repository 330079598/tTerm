import { invoke } from "@tauri-apps/api/core"

import type { TerminalSessionContext } from "@/components/TerminalTab/terminalLifecycleTypes"
import type { SavedPasswordPromptState, SudoPasswordSource } from "@/components/TerminalTab/types"
import { notifySavedPasswordNotSent } from "@/components/TerminalTab/savedPasswordNotice"
import { matchPasswordPrompt, readCursorLine, type PasswordPromptMatch } from "@/lib/sudoPrompt"
import { LoginScriptRunner, parseLoginScript } from "@/lib/loginScript"
import { nextTypedLength } from "@/lib/passwordFeedback"
import { setTabPasswordPrompt } from "@/lib/tabAttention"
import { EMPTY_COMMAND_CAPTURE_STATE } from "@/lib/terminalCommandCapture"

/**
 * The same prompt returning this soon after a fill means the password was
 * refused; sudo's failure delay is about two seconds.
 */
const PASSWORD_REJECT_WINDOW_MS = 20_000

/**
 * Offers the saved password when a sudo prompt shows up and fills it, and
 * types the connection's login script.
 */
export function createSavedPasswordPrompts(ctx: TerminalSessionContext) {
  const {
    term,
    session,
    tabId,
    sessionNonce,
    connectionRef,
    onSensitivePromptRef,
    onSavedPasswordPromptChange,
    setSavedPasswordPrompt,
    sudoPromptPatternsRef,
    onInputRef,
    translationRef,
    savedPasswordPromptActionsRef,
  } = ctx
  // Key of the prompt the user already answered (typed, dismissed, or
  // filled). sudo-rs echoes `*` per key, so erasing them brings the bare
  // prompt line back; that is the same prompt and must not be offered again.
  let answeredPromptKey: string | null = null
  let offeredPrompt: { prompt: string; user: string | null; source: SudoPasswordSource } | null =
    null
  // Set once a filled password is refused; stays for this session so a
  // stale password cannot burn through the remote's lockout budget.
  let savedPasswordRejected = false
  // The prompt under the cursor, until it is answered.
  let currentPrompt: PasswordPromptMatch | null = null
  // What the bar and the cursor overlay show, so typing counts on from it.
  let shown: SavedPasswordPromptState | null = null

  const showPrompt = (state: SavedPasswordPromptState | null) => {
    shown = state
    setSavedPasswordPrompt(state)
  }

  const showTyped = (match: PasswordPromptMatch, typed: number) =>
    showPrompt({ status: "manual", prompt: match.prompt, user: match.user, typed })

  const clearSavedPasswordOffer = () => {
    // Invalidates any in-flight source lookup so it cannot revive the offer.
    session.passwordPromptCheckId += 1
    if (offeredPrompt) {
      offeredPrompt = null
      onSavedPasswordPromptChange?.(tabId, sessionNonce, null)
    }
    showPrompt(null)
  }

  const answerCurrentPrompt = () => {
    if (session.currentPromptKey !== null) answeredPromptKey = session.currentPromptKey
    clearSavedPasswordOffer()
  }

  /** Closing the offer leaves the user to type the password. */
  const dismissCurrentPrompt = () => {
    const match = currentPrompt
    answerCurrentPrompt()
    if (match) showTyped(match, 0)
  }

  /**
   * The user types at the prompt themselves: an offer gives way to the
   * count of what they typed, which the cursor overlay shows until Enter or
   * Ctrl+C ends the prompt and clears the tab's key mark with it.
   */
  const typeAtPrompt = (data: string) => {
    const match = currentPrompt
    const typed = match ? nextTypedLength(shown?.status === "manual" ? shown.typed : 0, data) : null
    answerCurrentPrompt()
    if (match && typed !== null) {
      showTyped(match, typed)
    } else if (match) {
      currentPrompt = null
      setTabPasswordPrompt(tabId, false)
    }
  }

  const handlePasswordPrompt = (match: PasswordPromptMatch) => {
    session.commandCaptureSuspended = true
    session.commandCaptureState = EMPTY_COMMAND_CAPTURE_STATE
    const connection = connectionRef.current
    const profileId = connection?.profileId
    const profileName = connection?.profileName
    const typeManually = () => {
      onSensitivePromptRef.current?.(tabId)
      showTyped(match, 0)
    }

    const fill = session.lastPasswordFill
    session.lastPasswordFill = null
    if (fill && fill.prompt === match.prompt && Date.now() - fill.at < PASSWORD_REJECT_WINDOW_MS) {
      savedPasswordRejected = true
      showPrompt({ status: "rejected", prompt: match.prompt, user: match.user })
      onSensitivePromptRef.current?.(tabId)
      return
    }

    // A prompt naming another account (su, sudo -u with targetpw, a nested
    // login) must never receive this profile's password.
    const userMatches = match.user === null || match.user === connection?.username
    if (savedPasswordRejected || !userMatches || !(profileId || profileName)) {
      typeManually()
      return
    }

    const checkId = ++session.passwordPromptCheckId
    invoke<SudoPasswordSource | null>("get_sudo_password_source", { profileId, profileName })
      .then((source) => {
        if (session.disposed || checkId !== session.passwordPromptCheckId) return
        if (!source) {
          typeManually()
          return
        }
        offeredPrompt = { prompt: match.prompt, user: match.user, source }
        onSavedPasswordPromptChange?.(tabId, sessionNonce, match.prompt)
        showPrompt({ status: "available", ...offeredPrompt })
      })
      .catch((err) => {
        if (session.disposed || checkId !== session.passwordPromptCheckId) return
        console.error("Failed to look up saved sudo password:", err)
        typeManually()
      })
  }

  // Runs after each output chunk is parsed, on the line under the cursor, so
  // escape sequences, split chunks, and tmux redraws are already resolved.
  const checkPasswordPrompt = () => {
    if (session.disposed || connectionRef.current?.type !== "ssh") return
    const line = readCursorLine(term.buffer.active)
    const match = matchPasswordPrompt(
      line,
      connectionRef.current.username,
      sudoPromptPatternsRef.current
    )
    const key = match ? `${session.lineFeedCount}:${match.prompt}` : null
    if (key === session.currentPromptKey) return
    session.currentPromptKey = key
    currentPrompt = match
    setTabPasswordPrompt(tabId, match !== null)
    clearSavedPasswordOffer()
    if (match && key !== answeredPromptKey) handlePasswordPrompt(match)
  }

  const stopLoginScript = () => {
    session.loginScriptRunner?.stop()
    session.loginScriptRunner = null
  }

  const startLoginScript = () => {
    stopLoginScript()
    const connection = connectionRef.current
    if (connection?.type !== "ssh") return
    const lines = parseLoginScript(connection.loginScript)
    if (lines.length === 0) return
    session.loginScriptRunner = new LoginScriptRunner({
      lines,
      // Written straight to this session: a login script is never broadcast.
      send: (line) => {
        if (session.disposed) return
        invoke("write_pty", { tabId, sessionNonce, data: `${line}\r` }).catch(console.error)
      },
      readCursorLine: () => readCursorLine(term.buffer.active),
      isAtPasswordPrompt: () => session.currentPromptKey !== null,
    })
  }

  const fillSavedPassword = (): boolean => {
    const offer = offeredPrompt
    if (!offer) return false
    answerCurrentPrompt()
    session.commandCaptureState = EMPTY_COMMAND_CAPTURE_STATE
    session.commandCaptureSuspended = false

    // Only a write the backend accepted can be refused by the remote.
    const onWritten = (written: boolean | void) => {
      if (written && !session.disposed)
        session.lastPasswordFill = { prompt: offer.prompt, at: Date.now() }
    }
    const onInput = onInputRef.current
    if (onInput) {
      onInput({ tabId, sessionNonce, data: offer.prompt, kind: "saved-password" })
        .then(onWritten)
        .catch(console.error)
    } else {
      invoke<boolean>("write_saved_password_for_sudo", {
        tabId,
        sessionNonce,
        profileId: connectionRef.current?.profileId,
        profileName: connectionRef.current?.profileName,
        prompt: offer.prompt,
      })
        .then((written) => {
          if (!written) notifySavedPasswordNotSent(translationRef.current)
          onWritten(written)
        })
        .catch(console.error)
    }
    return true
  }

  savedPasswordPromptActionsRef.current = {
    fill: fillSavedPassword,
    dismiss: dismissCurrentPrompt,
    atPasswordPrompt: () => session.currentPromptKey !== null,
  }

  return { typeAtPrompt, checkPasswordPrompt, stopLoginScript, startLoginScript }
}

export type SavedPasswordPrompts = ReturnType<typeof createSavedPasswordPrompts>
