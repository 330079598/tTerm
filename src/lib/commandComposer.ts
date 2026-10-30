import { useSyncExternalStore } from "react"

// The compose bar: text edited locally and sent to the focused terminal in
// one go, so that over a slow link typing never waits on the round trip.

const OPEN_STORAGE_KEY = "tterm.command-composer.open"
const PASTE_START = "\x1b[200~"
const PASTE_END = "\x1b[201~"

/** A terminal the compose bar can send to. */
export interface ComposerTarget {
  /**
   * Sends data the way typing does, through the terminal's own input, so the
   * command is noted, timed and broadcast like a typed one.
   */
  input: (data: string) => void
  /** The program asked for bracketed paste, as shells do at their prompt. */
  bracketedPasteMode: () => boolean
}

/**
 * What to send for `text`, run at once: one line followed by Enter, or
 * several pasted the way a paste goes in, then Enter. Line breaks become CR,
 * as Enter types it. An empty text is a bare Enter.
 */
export function composerInput(text: string, bracketedPaste: boolean): string {
  const lines = text.replace(/\r\n?/g, "\n").replace(/\n+$/, "").replace(/\n/g, "\r")
  if (!lines.includes("\r") || !bracketedPaste) return `${lines}\r`
  // An end marker inside the text would end the paste early and run the rest.
  return `${PASTE_START}${lines.split(PASTE_END).join("")}${PASTE_END}\r`
}

/** No line break comes before the caret, so ArrowUp would not move it. */
export function caretOnFirstLine(text: string, caret: number) {
  return !text.slice(0, caret).includes("\n")
}

/** No line break comes after the caret, so ArrowDown would not move it. */
export function caretOnLastLine(text: string, caret: number) {
  return !text.slice(caret).includes("\n")
}

const targets = new Map<string, ComposerTarget>()

/** Makes tab `tabId`'s terminal reachable from the bar, until the returned function runs. */
export function registerComposerTarget(tabId: string, target: ComposerTarget) {
  targets.set(tabId, target)
  return () => {
    // A reconnect registers the new session before the old one goes.
    if (targets.get(tabId) === target) targets.delete(tabId)
  }
}

export function getComposerTarget(tabId: string) {
  return targets.get(tabId)
}

function loadOpen() {
  try {
    return localStorage.getItem(OPEN_STORAGE_KEY) === "1"
  } catch {
    return false
  }
}

let state = { open: loadOpen(), focusRequest: 0 }
const listeners = new Set<() => void>()

function setState(next: typeof state) {
  state = next
  try {
    localStorage.setItem(OPEN_STORAGE_KEY, next.open ? "1" : "0")
  } catch {
    // The bar still opens; it only won't be open at the next start.
  }
  for (const listener of listeners) listener()
}

/** Shows the bar and puts the keyboard in it. */
export function openComposer() {
  setState({ open: true, focusRequest: state.focusRequest + 1 })
}

export function closeComposer() {
  if (state.open) setState({ ...state, open: false })
}

export function isComposerOpen() {
  return state.open
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

/** Whether the bar is shown, and a count that goes up each time it should take the keyboard. */
export function useComposerState() {
  return useSyncExternalStore(subscribe, () => state)
}
