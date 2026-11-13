/** Poll infrequently; only a long scheduling gap is treated as a possible resume. */
const CHECK_INTERVAL_MS = 2_000
const RESUME_GAP_MS = 10_000

const listeners = new Set<() => void>()
let stopWatching: (() => void) | undefined

/**
 * Shares wake detection across visible panes. A display wake may only change
 * visibility, while system sleep can leave visibility and focus unchanged.
 * The clock-gap fallback covers the latter without a per-terminal timer.
 */
export function subscribeWindowResume(listener: () => void): () => void {
  listeners.add(listener)
  stopWatching ??= watchWindowResume()
  return () => {
    listeners.delete(listener)
    if (listeners.size === 0) {
      stopWatching?.()
      stopWatching = undefined
    }
  }
}

function watchWindowResume(): () => void {
  let lastCheck = Date.now()
  let frame: number | undefined
  const schedule = () => {
    if (document.visibilityState === "hidden" || frame !== undefined) return
    frame = window.requestAnimationFrame(() => {
      frame = undefined
      if (document.visibilityState === "hidden") return
      lastCheck = Date.now()
      for (const listener of listeners) listener()
    })
  }
  const checkClock = () => {
    const now = Date.now()
    if (now - lastCheck > RESUME_GAP_MS) schedule()
    lastCheck = now
  }
  const timer = window.setInterval(checkClock, CHECK_INTERVAL_MS)
  window.addEventListener("focus", schedule)
  window.addEventListener("pageshow", schedule)
  document.addEventListener("visibilitychange", schedule)
  return () => {
    window.clearInterval(timer)
    if (frame !== undefined) window.cancelAnimationFrame(frame)
    window.removeEventListener("focus", schedule)
    window.removeEventListener("pageshow", schedule)
    document.removeEventListener("visibilitychange", schedule)
  }
}
