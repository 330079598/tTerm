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
    // Without storage the next launch starts classic until the config loads.
  }
}

/** Applies the style remembered from the last run, if there is one. */
export function preloadUiStyle(): void {
  try {
    if (localStorage.getItem(UI_STYLE_CACHE_KEY) === "islands") {
      document.documentElement.classList.add(ISLANDS_CLASS)
    }
  } catch {
    // Nothing remembered; the config applies the style once it loads.
  }
}
