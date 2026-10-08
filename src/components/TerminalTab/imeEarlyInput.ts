import type { IDisposable, Terminal } from "@xterm/xterm"

/**
 * Keeps IME text that arrives before its keydown from being dropped.
 *
 * WebKit (WKWebView on macOS) fires the IME's insertText input event for
 * punctuation and digits before the keydown of the same keystroke. xterm has
 * two paths for such text: its keydown handler diffs the textarea after a
 * timeout, but the text is already there when keydown snapshots it, so the
 * diff is empty; and its input handler skips events while `_keyDownSeen` is
 * set, which any keydown sets until the next keyup. So the text is lost
 * whenever another key is still down: a held Shift ("？", "！", "："), the
 * previous key during fast typing, or Caps Lock, which macOS reports as down
 * for as long as it is on.
 *
 * Chromium sends the keydown (keyCode 229) first and xterm's diff picks the
 * text up. So an input that arrives while no such diff is pending came first,
 * and the fix clears the flag just before xterm's input handler sees it.
 *
 * It patches an xterm 6 private member; if it is missing the fix is a no-op.
 */

/** keyCode of a keydown the IME is processing. */
const IME_KEY_CODE = 229

interface CoreInternals {
  _keyDownSeen: boolean
  _compositionHelper?: { readonly isComposing: boolean; _isSendingComposition?: boolean }
}

const NOOP_DISPOSABLE: IDisposable = { dispose: () => {} }

export function installImeEarlyInputFix(term: Terminal): IDisposable {
  const core = (term as unknown as { _core?: CoreInternals })._core
  const textarea = term.textarea
  const root = term.element
  if (
    !core ||
    typeof core._keyDownSeen !== "boolean" ||
    !core._compositionHelper ||
    !textarea ||
    !root
  ) {
    return NOOP_DISPOSABLE
  }
  const helper = core._compositionHelper

  // xterm diffs the textarea one timeout after an IME keydown; until then the
  // keystroke's text belongs to that diff.
  let pendingDiffs = 0
  const onKeyDown = (event: KeyboardEvent) => {
    if (event.keyCode !== IME_KEY_CODE) return
    pendingDiffs += 1
    setTimeout(() => {
      pendingDiffs -= 1
    }, 0)
  }

  // Runs in the capture phase on the terminal element, before xterm's own
  // listener on the textarea.
  const onInput = (event: Event) => {
    const { inputType, data, isComposing } = event as InputEvent
    if (event.target !== textarea || pendingDiffs > 0 || isComposing) return
    if (inputType !== "insertText" || !data) return
    if (helper.isComposing || helper._isSendingComposition) return
    core._keyDownSeen = false
  }

  root.addEventListener("keydown", onKeyDown, true)
  root.addEventListener("input", onInput, true)

  return {
    dispose: () => {
      root.removeEventListener("keydown", onKeyDown, true)
      root.removeEventListener("input", onInput, true)
    },
  }
}
