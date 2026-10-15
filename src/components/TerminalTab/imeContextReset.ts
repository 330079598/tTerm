import type { IDisposable, Terminal } from "@xterm/xterm"

/**
 * Empties xterm's hidden textarea once a key leaves the text the IME last saw.
 *
 * The OS IME reads the text before the caret in that textarea as context. The
 * macOS Pinyin IME uses it to put a space between Chinese and a digit or Latin
 * letter that follows, so after "是的" a typed "2" arrives as " 2". xterm keeps
 * everything typed in the textarea and empties it only on Enter and Ctrl+C, so
 * after Ctrl+J, which Claude Code and Codex use for a new line, the IME still
 * sees "是的" and the next line starts with a space. Arrow keys, Tab,
 * Backspace and Escape leave the same stale context behind.
 *
 * So whenever xterm sends a control key from a keydown, the textarea is emptied
 * the way xterm does for Enter. Printable keys keep the context, so spacing
 * within a line behaves as in any other text field.
 */

/** keyCode of a keydown the IME is processing. */
const IME_KEY_CODE = 229

const NOOP_DISPOSABLE: IDisposable = { dispose: () => {} }

/** Whether xterm's output for a key is a control character or sequence. */
export function isControlKey(key: string): boolean {
  const code = key.charCodeAt(0)
  return code < 0x20 || code === 0x7f
}

export function installImeContextReset(term: Terminal): IDisposable {
  const textarea = term.textarea
  const root = term.element
  if (!textarea || !root) return NOOP_DISPOSABLE

  // xterm diffs the textarea one timeout after an IME keydown; emptying it
  // before then would read as a deletion and send DEL.
  let pendingDiffs = 0
  const onKeyDown = (event: KeyboardEvent) => {
    if (event.keyCode !== IME_KEY_CODE) return
    pendingDiffs += 1
    setTimeout(() => {
      pendingDiffs -= 1
    }, 0)
  }
  root.addEventListener("keydown", onKeyDown, true)

  // xterm fires onKey only once its composition handling has finished, so no
  // composition is reading from the textarea at this point.
  const keys = term.onKey(({ key }) => {
    if (pendingDiffs > 0 || !isControlKey(key)) return
    textarea.value = ""
  })

  return {
    dispose: () => {
      root.removeEventListener("keydown", onKeyDown, true)
      keys.dispose()
    },
  }
}
