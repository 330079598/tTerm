import { platform } from "@tauri-apps/plugin-os"

import type { AppConfig } from "@/lib/appConfig"

export function applyUiScalePercent(scale: number) {
  const rootStyle = document.documentElement.style
  const factor = scale / 100
  rootStyle.setProperty("--ui-font-scale", String(factor))
  rootStyle.setProperty("--text-xs", `${0.75 * factor}rem`)
  rootStyle.setProperty("--text-xs--line-height", "1.15")
  rootStyle.setProperty("--text-sm", `${0.875 * factor}rem`)
  rootStyle.setProperty("--text-sm--line-height", "1.2")
  rootStyle.setProperty("--text-base", `${factor}rem`)
  rootStyle.setProperty("--text-base--line-height", "1.25")
  rootStyle.setProperty("--text-lg", `${1.125 * factor}rem`)
  rootStyle.setProperty("--text-lg--line-height", "1.25")
  rootStyle.setProperty("--text-xl", `${1.25 * factor}rem`)
  rootStyle.setProperty("--text-xl--line-height", "1.25")
  rootStyle.setProperty("--text-2xl", `${1.5 * factor}rem`)
  rootStyle.setProperty("--text-2xl--line-height", "1.2")
  rootStyle.setProperty("--text-3xl", `${1.875 * factor}rem`)
  rootStyle.setProperty("--text-3xl--line-height", "1.2")
}

let _cachedPlatform: string | null = null

export function getDetectedPlatform(): string {
  try {
    _cachedPlatform ??= platform()
    return _cachedPlatform
  } catch {
    if (typeof navigator !== "undefined") {
      const hint = `${navigator.platform} ${navigator.userAgent}`.toLowerCase()
      if (hint.includes("mac")) return "macos"
      if (hint.includes("win")) return "windows"
      if (hint.includes("linux")) return "linux"
    }
    return "unknown"
  }
}

/** The blur needs the window system's help; Linux leaves it to the compositor. */
export function isWindowBlurSupported(): boolean {
  const detected = getDetectedPlatform()
  return detected === "macos" || detected === "windows"
}

export function isWindowBlurEnabled(config: Pick<AppConfig, "window_blur">): boolean {
  return config.window_blur && isWindowBlurSupported()
}
