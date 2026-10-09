// Lets code outside a terminal tab, such as a notification leading back to
// it, put the keyboard focus in the tab's terminal.
const focusers = new Map<string, () => void>()

export function registerTerminalFocus(tabId: string, focus: () => void): () => void {
  focusers.set(tabId, focus)
  return () => {
    if (focusers.get(tabId) === focus) focusers.delete(tabId)
  }
}

export function focusTerminal(tabId: string) {
  focusers.get(tabId)?.()
}
