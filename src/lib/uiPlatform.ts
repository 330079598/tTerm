import { platform } from "@tauri-apps/plugin-os"

import type { AppConfig } from "@/lib/appConfig"
import tokens from "@/styles/tokens.json"

/** Writes the type scale from src/styles/tokens.json, scaled by the factor. */
export function applyUiScalePercent(scale: number) {
  const rootStyle = document.documentElement.style
  const factor = scale / 100
  rootStyle.setProperty("--ui-font-scale", String(factor))
  for (const [name, size] of Object.entries(tokens.fontSizes)) {
    rootStyle.setProperty(`--text-${name}`, `${size.rem * factor}rem`)
    rootStyle.setProperty(`--text-${name}--line-height`, String(size.lineHeight))
  }
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
