import { useEffect, useState } from "react"

/** How long the words stay before only the Caps Lock mark is left. */
export const CAPS_LOCK_NOTICE_MS = 2500

/**
 * Whether to spell out that Caps Lock is on beside the cursor. The mark that
 * replaces the key is easy to miss the first time, so the words show for a
 * moment when Caps Lock turns on, or is already on as the prompt appears.
 * Typing does not cut them short: the key that reveals Caps Lock is often the
 * first one typed, and with Caps Lock on the stars count a wrong password.
 */
export function useCapsLockNotice(capsLockOn: boolean): boolean {
  const [shown, setShown] = useState(capsLockOn)
  const [lastCapsLockOn, setLastCapsLockOn] = useState(capsLockOn)
  if (capsLockOn !== lastCapsLockOn) {
    setLastCapsLockOn(capsLockOn)
    setShown(capsLockOn)
  }

  useEffect(() => {
    if (!shown) return
    const timer = window.setTimeout(() => setShown(false), CAPS_LOCK_NOTICE_MS)
    return () => window.clearTimeout(timer)
  }, [shown])

  return shown
}
