import { useSyncExternalStore } from "react"

function subscribe(listener: () => void) {
  // Without capture, these fire only when the window itself gains or loses
  // focus, not when focus moves between elements inside it.
  window.addEventListener("focus", listener)
  window.addEventListener("blur", listener)
  return () => {
    window.removeEventListener("focus", listener)
    window.removeEventListener("blur", listener)
  }
}

/** Whether the app's window has the keyboard, rather than another app. */
export function useWindowFocused(): boolean {
  return useSyncExternalStore(
    subscribe,
    () => document.hasFocus(),
    () => true
  )
}
