import { DEFAULT_TAB_ICON_CONFIG, normalizeTabIconConfig, type TabIconConfig } from "@/lib/tabIcons"

/** How wide tabs are: each as wide as its title, or all the same width. */
export type TabWidthMode = "adaptive" | "standard"

/** The settings for how tabs look, part of the app config. */
export interface TabSettings {
  tab_width_mode: TabWidthMode
  tab_standard_width: number
  tab_icons: TabIconConfig
}

export const DEFAULT_TAB_SETTINGS: TabSettings = {
  tab_width_mode: "adaptive",
  tab_standard_width: 120,
  tab_icons: DEFAULT_TAB_ICON_CONFIG,
}

/** The tab settings from a saved config, each one valid. */
export function normalizeTabSettings(
  config: Partial<Record<keyof TabSettings, unknown>>
): TabSettings {
  return {
    tab_width_mode: config.tab_width_mode === "standard" ? "standard" : "adaptive",
    tab_standard_width: normalizeTabStandardWidth(config.tab_standard_width),
    tab_icons: normalizeTabIconConfig(config.tab_icons),
  }
}

/** A standard tab width in pixels, rounded into 80-300; 120 when unset. */
function normalizeTabStandardWidth(value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return DEFAULT_TAB_SETTINGS.tab_standard_width
  }

  return Math.min(Math.max(Math.round(value), 80), 300)
}
