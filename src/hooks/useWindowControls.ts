import { useCallback, useMemo } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"

/** Width of each native window button, matching `button_width` in lib.rs. */
const NATIVE_BUTTON_WIDTH_PX = 46

/**
 * How far the title bar's own buttons may slide under the native buttons. The
 * native glyphs sit in wide empty padding, so tucking the settings button
 * beneath them closes the gap. Past 6px the icon's right edge enters the
 * minimize button's hit area, which is accepted for a tighter group.
 */
const NATIVE_CONTROLS_OVERLAP_PX = 10

export function useWindowControls(isWindows: boolean) {
  const nativeControlsReservePx = useMemo(
    () => (isWindows ? NATIVE_BUTTON_WIDTH_PX * 3 - NATIVE_CONTROLS_OVERLAP_PX : 0),
    [isWindows]
  )

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
