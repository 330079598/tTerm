import type { UiStyle } from "@/lib/appConfig"

const UI_STYLE_CACHE_KEY = "tterm-ui-style"
const ISLANDS_CLASS = "ui-islands"

/**
 * Puts the window in `style` and remembers it, so the next launch starts in
 * it before the config loads instead of jumping from the classic layout.
 */
export function applyUiStyle(style: UiStyle): void {
  document.documentElement.classList.toggle(ISLANDS_CLASS, style === "islands")
  try {
    localStorage.setItem(UI_STYLE_CACHE_KEY, style)
  } catch {
    // Without storage the next launch starts in islands until the config loads.
  }
}

/**
 * Applies the style remembered from the last run. With nothing remembered this
 * is most likely a first launch, which starts in islands like a new config.
 */
export function preloadUiStyle(): void {
  let remembered: string | null = null
  try {
    remembered = localStorage.getItem(UI_STYLE_CACHE_KEY)
  } catch {
    // Storage is unavailable; the config corrects the style once it loads.
  }
  document.documentElement.classList.toggle(ISLANDS_CLASS, remembered !== "classic")
}
