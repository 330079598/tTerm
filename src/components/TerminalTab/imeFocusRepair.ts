import type { IDisposable, Terminal } from "@xterm/xterm"

/**
 * Re-attaches the OS IME to the terminal when WebView2 loses track of it.
 *
 * After the window is re-activated, the Windows IME (TSF) sometimes stays
 * detached from the focused textarea (see src-tauri/src/ime_focus.rs). Keys
 * still arrive as "Process", but the IME composes on its own: no
 * compositionstart fires, the candidate window has no caret to anchor to and
 * sits in the screen's top-left corner, and the result is committed as plain
 * insertText characters.
 *
 * Moving DOM focus does not help, as TSF tracks native focus. The repair asks
 * the host to move native focus off the WebView and back. It runs shortly
 * after the window is activated, and whenever an ideograph is inserted outside
 * a composition, which only happens while the IME is detached.
 */

/** Delay before repairing, so xterm's own post-keydown textarea diff runs first. */
export const REPAIR_DELAY_MS = 50
/** Delay after window activation, so the repair lands after TSF re-targets focus. */
export const ACTIVATION_SETTLE_MS = 150
/**
 * How long the repair's own focus churn lasts. Window focus events in this
 * window are its echo, and focus reports are held back from the app.
 */
export const REPAIR_QUIET_MS = 500

// CJK ideographs, kana and hangul: IMEs only ever produce these by composing.
const COMPOSED_SCRIPT = /[぀-ヿ㐀-䶿一-鿿가-힯豈-﫿]/

interface FocusReportInternals {
  coreService?: { decPrivateModes?: { sendFocus?: boolean } }
}

const NOOP_DISPOSABLE: IDisposable = { dispose: () => {} }

export function installImeFocusRepair(
  term: Terminal,
  repairNativeFocus: () => Promise<unknown>,
  win: Window = window
): IDisposable {
  const textarea = term.textarea
  if (!textarea) return NOOP_DISPOSABLE
  const doc = textarea.ownerDocument
  const modes = (term as unknown as { _core?: FocusReportInternals })._core?.coreService
    ?.decPrivateModes

  let composing = false
  let repairTimer: ReturnType<typeof setTimeout> | null = null
  let quietTimer: ReturnType<typeof setTimeout> | null = null
  let heldFocusReports = false

  // xterm reports focus changes to apps that ask for them (DECSET 1004). The
  // repair's blur and refocus are not real ones, so keep them from the app.
  const endQuiet = () => {
    quietTimer = null
    if (heldFocusReports && modes) modes.sendFocus = true
    heldFocusReports = false
  }

  const repair = (reason: string) => {
    repairTimer = null
    if (composing || quietTimer !== null || doc.activeElement !== textarea) return
    console.debug(`[ime] re-attaching the IME to the terminal (${reason})`)
    if (modes?.sendFocus) {
      modes.sendFocus = false
      heldFocusReports = true
    }
    quietTimer = setTimeout(endQuiet, REPAIR_QUIET_MS)
    void repairNativeFocus().catch((error) => {
      console.warn("[ime] failed to re-attach the IME:", error)
    })
  }

  const scheduleRepair = (reason: string, delay: number) => {
    if (repairTimer === null) repairTimer = setTimeout(() => repair(reason), delay)
  }

  const onWindowFocus = () => {
    if (quietTimer !== null) return
    scheduleRepair("window activated", ACTIVATION_SETTLE_MS)
  }
  const onCompositionStart = () => {
    composing = true
  }
  const onCompositionEnd = () => {
    composing = false
  }
  const onInput = (event: Event) => {
    const { inputType, data } = event as InputEvent
    if (composing || inputType !== "insertText" || !data || !COMPOSED_SCRIPT.test(data)) return
    scheduleRepair("input arrived without a composition", REPAIR_DELAY_MS)
  }

  textarea.addEventListener("compositionstart", onCompositionStart)
  textarea.addEventListener("compositionend", onCompositionEnd)
  textarea.addEventListener("input", onInput)
  win.addEventListener("focus", onWindowFocus)

  return {
    dispose: () => {
      if (repairTimer !== null) clearTimeout(repairTimer)
      if (quietTimer !== null) {
        clearTimeout(quietTimer)
        endQuiet()
      }
      textarea.removeEventListener("compositionstart", onCompositionStart)
      textarea.removeEventListener("compositionend", onCompositionEnd)
      textarea.removeEventListener("input", onInput)
      win.removeEventListener("focus", onWindowFocus)
    },
  }
}
