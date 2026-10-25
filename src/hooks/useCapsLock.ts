import { useSyncExternalStore } from "react"

let capsLockOn = false
const listeners = new Set<() => void>()
let installed = false

function update(event: KeyboardEvent | PointerEvent) {
  const next = event.getModifierState("CapsLock")
  if (next === capsLockOn) return
  capsLockOn = next
  listeners.forEach((listener) => listener())
}

function subscribe(listener: () => void) {
  // One window-wide listener stays installed, so the state is current even
  // for a field that mounts after the last key press.
  if (!installed) {
    installed = true
    // Capture phase: xterm stops the keys it handles from bubbling.
    window.addEventListener("keydown", update, true)
    window.addEventListener("keyup", update, true)
    window.addEventListener("pointerdown", update, true)
  }
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

/**
 * Whether Caps Lock is on, as of the last key press or click in the window.
 * Browsers cannot read it before then, so it starts out false.
 */
export function useCapsLock(): boolean {
  return useSyncExternalStore(
    subscribe,
    () => capsLockOn,
    () => false
  )
}
