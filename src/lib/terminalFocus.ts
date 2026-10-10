import type React from "react"

// Lets code outside a terminal tab, such as a notification leading back to
// it, put the keyboard focus in the tab's terminal.
const focusers = new Map<string, () => void>()

// The focused pane of the tab in sight, while that tab is a terminal.
let activeTabId: string | null = null

export function registerTerminalFocus(tabId: string, focus: () => void): () => void {
  focusers.set(tabId, focus)
  return () => {
    if (focusers.get(tabId) === focus) focusers.delete(tabId)
  }
}

export function focusTerminal(tabId: string) {
  focusers.get(tabId)?.()
}

export function markActiveTerminal(tabId: string): () => void {
  activeTabId = tabId
  return () => {
    if (activeTabId === tabId) activeTabId = null
  }
}

export function focusActiveTerminal() {
  if (activeTabId) focusTerminal(activeTabId)
}

function focusIsLost() {
  const focused = document.activeElement
  return !focused || focused === document.body
}

// A menu or panel that held the focus and closed leaves it nowhere, so typing
// goes nowhere until the terminal is clicked; hand it to the terminal instead.
export function refocusTerminalIfFocusLost() {
  if (focusIsLost()) focusActiveTerminal()
}

// Gives the focus back when an overlay closes: to where it was when the
// overlay opened, or to the terminal in sight when that place is gone, was
// nowhere, or can no longer take focus (inside a closed SFTP drawer).
export function restoreFocus(previous: Element | null) {
  if (previous instanceof HTMLElement && previous.isConnected && previous !== document.body) {
    previous.focus()
    if (document.activeElement === previous) return
  }
  focusActiveTerminal()
}

// For window chrome, such as tabs and toolbar buttons: a click on it acts but
// leaves the focus where it was, in the terminal, rather than on itself.
export function keepFocusOnMouseDown(event: React.MouseEvent) {
  event.preventDefault()
}
