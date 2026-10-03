import { useEffect, useState, useSyncExternalStore } from "react"
import { invoke, isTauri } from "@tauri-apps/api/core"

import { getDetectedPlatform } from "@/contexts/ConfigContext"

const DARK_QUERY = "(prefers-color-scheme: dark)"
/** How often Windows is asked; it tells the page nothing when its mode changes. */
const WINDOWS_POLL_MS = 3000

function mediaPrefersDark(): boolean {
  return window.matchMedia?.(DARK_QUERY).matches === true
}

function subscribeToMedia(onChange: () => void) {
  const media = window.matchMedia?.(DARK_QUERY)
  media?.addEventListener("change", onChange)
  return () => media?.removeEventListener("change", onChange)
}

/** On Windows the web view's color scheme follows the window, which the blur pins to the page's. */
function asksSystem(): boolean {
  return isTauri() && getDetectedPlatform() === "windows"
}

/**
 * Whether the system is in dark mode. On Windows the backend is asked while
 * `active`, and the answer is `null` until it comes.
 */
export function useSystemPrefersDark(active: boolean): boolean | null {
  const mediaDark = useSyncExternalStore(subscribeToMedia, mediaPrefersDark, () => false)
  const [windowsDark, setWindowsDark] = useState<boolean | null>(null)

  useEffect(() => {
    if (!active || !asksSystem()) return

    let cancelled = false
    const ask = () =>
      invoke<boolean | null>("system_prefers_dark")
        .then((dark) => {
          if (!cancelled) setWindowsDark(dark ?? mediaPrefersDark())
        })
        .catch((error: unknown) => {
          console.error("Failed to read the system appearance:", error)
          if (!cancelled) setWindowsDark(mediaPrefersDark())
        })
    void ask()
    const timer = window.setInterval(ask, WINDOWS_POLL_MS)
    window.addEventListener("focus", ask)
    return () => {
      cancelled = true
      window.clearInterval(timer)
      window.removeEventListener("focus", ask)
    }
  }, [active])

  return asksSystem() ? windowsDark : mediaDark
}
