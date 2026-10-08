import type { IDisposable, Terminal } from "@xterm/xterm"

/**
 * Keeps IME punctuation typed with a held modifier from being dropped.
 *
 * WebKit (WKWebView on macOS) fires the IME's insertText input event before
 * the keydown of the same keystroke. xterm has two paths for it: its keydown
 * handler diffs the textarea after a timeout, but the text is already there
 * when keydown snapshots it, so the diff is empty; and its input handler skips
 * events while `_keyDownSeen` is set, which any keydown sets until the next
 * keyup, a held Shift included. So the first "？" or "！" after pressing Shift
 * is lost, and the next one only arrives because the key's keyup cleared the
 * flag.
 *
 * An input that arrives while only modifiers are down belongs to no keydown
 * xterm is handling, so the fix clears the flag just before xterm sees it.
 * Chromium sends keydown first, which marks a key as down and leaves xterm's
 * own diff to send the text.
 *
 * It patches an xterm 6 private member; if it is missing the fix is a no-op.
 */

const MODIFIER_KEYS = new Set(["Shift", "Control", "Alt", "Meta"])

interface CoreInternals {
  _keyDownSeen: boolean
  _compositionHelper?: { readonly isComposing: boolean; _isSendingComposition?: boolean }
}

const NOOP_DISPOSABLE: IDisposable = { dispose: () => {} }

export function installImeModifierInputFix(term: Terminal): IDisposable {
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

  // Mirrors xterm's own bookkeeping: any keyup ends the keystroke, since macOS
  // sends no keyup for a key released while Cmd is held.
  let keyDown = false
  const onKeyDown = (event: KeyboardEvent) => {
    if (!MODIFIER_KEYS.has(event.key)) keyDown = true
  }
  const onKeyUp = () => {
    keyDown = false
  }

  // Runs in the capture phase on the terminal element, before xterm's own
  // listener on the textarea.
  const onInput = (event: Event) => {
    const { inputType, data, isComposing } = event as InputEvent
    if (event.target !== textarea || keyDown || isComposing) return
    if (inputType !== "insertText" || !data) return
    if (helper.isComposing || helper._isSendingComposition) return
    core._keyDownSeen = false
  }

  root.addEventListener("keydown", onKeyDown, true)
  root.addEventListener("keyup", onKeyUp, true)
  root.addEventListener("input", onInput, true)
  textarea.addEventListener("blur", onKeyUp)

  return {
    dispose: () => {
      root.removeEventListener("keydown", onKeyDown, true)
      root.removeEventListener("keyup", onKeyUp, true)
      root.removeEventListener("input", onInput, true)
      textarea.removeEventListener("blur", onKeyUp)
    },
  }
}
