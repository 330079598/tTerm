import { useCallback, useMemo } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"

export function useWindowControls(isWindows: boolean) {
  const nativeControlsReservePx = useMemo(() => (isWindows ? 46 * 3 : 0), [isWindows])

  const handleMinimizeWindow = useCallback(() => {
    void getCurrentWindow().minimize().catch(console.error)
  }, [])

  const handleToggleMaximizeWindow = useCallback(() => {
    void getCurrentWindow().toggleMaximize().catch(console.error)
  }, [])

  const handleCloseWindow = useCallback(() => {
    void getCurrentWindow().close().catch(console.error)
  }, [])

  return {
    nativeControlsReservePx,
    handleMinimizeWindow,
    handleToggleMaximizeWindow,
    handleCloseWindow,
  }
}
