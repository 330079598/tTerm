import { useEffect, useRef } from "react"
import { invoke } from "@tauri-apps/api/core"

/**
 * Starts tunnels marked "start automatically" once the app is ready to use
 * saved secrets. The backend runs this only once per launch, so re-triggering
 * (e.g. after a window reload) never revives tunnels the user stopped.
 */
export function useTunnelAutoStart(ready: boolean): void {
  const requestedRef = useRef(false)

  useEffect(() => {
    if (!ready || requestedRef.current) return
    requestedRef.current = true
    invoke("auto_start_tunnels").catch((error) =>
      console.error("Failed to auto-start tunnels:", error)
    )
  }, [ready])
}
