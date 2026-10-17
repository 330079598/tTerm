import { useEffect } from "react"

import type { AppConfig } from "@/lib/appConfig"
import { setSpinnerStyle } from "@/lib/spinnerStyle"
import { setWindowBlur } from "@/lib/themePreloader"
import { applyUiScalePercent, isWindowBlurEnabled } from "@/lib/uiPlatform"

/** Applies the UI scale, the window blur and the loading spinners' style from the config. */
export function useWindowAppearance(config: AppConfig, isLoaded: boolean) {
  const spinnerStyle = config.tab_agent_indicator.working
  useEffect(() => setSpinnerStyle(spinnerStyle), [spinnerStyle])

  useEffect(() => {
    applyUiScalePercent(config.ui_scale_percent)
    return () => {
      const rootStyle = document.documentElement.style
      rootStyle.removeProperty("--ui-font-scale")
      for (const size of ["xs", "sm", "base", "lg", "xl", "2xl", "3xl"]) {
        rootStyle.removeProperty(`--text-${size}`)
        rootStyle.removeProperty(`--text-${size}--line-height`)
      }
    }
  }, [config.ui_scale_percent])

  // Not before the config loads: the page starts with whatever the window was
  // created with, and the defaults would switch the blur off in between.
  const windowBlurEnabled = isWindowBlurEnabled(config)
  useEffect(() => {
    if (!isLoaded) return
    setWindowBlur({
      enabled: windowBlurEnabled,
      radius: config.window_blur_radius,
      material: config.window_blur_material,
      opacity: config.window_opacity_percent / 100,
    })
  }, [
    config.window_blur_material,
    config.window_blur_radius,
    config.window_opacity_percent,
    isLoaded,
    windowBlurEnabled,
  ])
}
