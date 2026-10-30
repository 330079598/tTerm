import { useSyncExternalStore } from "react"

import type { PromptEvent, ShellPromptState } from "@/components/TerminalTab/commandMarks"

// The compose bar: text edited locally and sent to the focused terminal in
// one go, so that over a slow link typing never waits on the round trip.

const OPEN_STORAGE_KEY = "tterm.command-composer.open"
const LINE_BY_LINE_STORAGE_KEY = "tterm.command-composer.line-by-line"
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
  /** Whether the shell marks its prompts (OSC 133 A) and waits at one now. */
  promptState: () => ShellPromptState
  /** Calls `listener` each time the shell is back at its prompt, until the returned function runs. */
  onPrompt: (listener: (event: PromptEvent) => void) => () => void
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

function loadFlag(key: string) {
  try {
    return localStorage.getItem(key) === "1"
  } catch {
    return false
  }
}

function saveFlag(key: string, value: boolean) {
  try {
    localStorage.setItem(key, value ? "1" : "0")
  } catch {
    // The choice still holds; it only won't be remembered at the next start.
  }
}

let state = {
  open: loadFlag(OPEN_STORAGE_KEY),
  focusRequest: 0,
  /** Several lines run one at a time rather than go in as one paste. */
  lineByLine: loadFlag(LINE_BY_LINE_STORAGE_KEY),
}
const listeners = new Set<() => void>()

function setState(next: typeof state) {
  state = next
  saveFlag(OPEN_STORAGE_KEY, next.open)
  saveFlag(LINE_BY_LINE_STORAGE_KEY, next.lineByLine)
  for (const listener of listeners) listener()
}

/** Shows the bar and puts the keyboard in it. */
export function openComposer() {
  setState({ ...state, open: true, focusRequest: state.focusRequest + 1 })
}

export function closeComposer() {
  if (state.open) setState({ ...state, open: false })
}

export function setComposerLineByLine(lineByLine: boolean) {
  setState({ ...state, lineByLine })
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

/**
 * Whether the bar is shown, a count that goes up each time it should take
 * the keyboard, and how it sends several lines.
 */
export function useComposerState() {
  return useSyncExternalStore(subscribe, () => state)
}
