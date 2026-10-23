import {
  DEFAULT_TAB_AGENT_INDICATOR,
  normalizeTabAgentIndicator,
  type TabAgentIndicatorConfig,
} from "@/lib/tabAgentIndicator"
import { DEFAULT_TAB_ICON_CONFIG, normalizeTabIconConfig, type TabIconConfig } from "@/lib/tabIcons"

/** How wide tabs are: each as wide as its title, or all the same width. */
export type TabWidthMode = "adaptive" | "standard"

/** How much the panes without the keyboard fade, in percent; 0 leaves them be. */
export const INACTIVE_PANE_DIM_OPTIONS = [0, 15, 30, 45] as const

/** How the panes of a split tab look. */
export interface PaneSettings {
  /** Each pane of a split tab shows a header naming it, with buttons to split, zoom and close it. */
  show_headers: boolean
  dim_inactive_percent: number
}

/** The settings for how tabs look, part of the app config. */
export interface TabSettings {
  tab_width_mode: TabWidthMode
  tab_standard_width: number
  /** Show a tab's full title and connection in a tooltip on hover. */
  tab_hover_tooltip: boolean
  tab_icons: TabIconConfig
  tab_agent_indicator: TabAgentIndicatorConfig
  panes: PaneSettings
}

export const DEFAULT_TAB_SETTINGS: TabSettings = {
  tab_width_mode: "adaptive",
  tab_standard_width: 120,
  tab_hover_tooltip: false,
  tab_icons: DEFAULT_TAB_ICON_CONFIG,
  tab_agent_indicator: DEFAULT_TAB_AGENT_INDICATOR,
  panes: { show_headers: true, dim_inactive_percent: 0 },
}

/** The tab settings from a saved config, each one valid. */
export function normalizeTabSettings(
  config: Partial<Record<keyof TabSettings, unknown>>
): TabSettings {
  return {
    tab_width_mode: config.tab_width_mode === "standard" ? "standard" : "adaptive",
    tab_standard_width: normalizeTabStandardWidth(config.tab_standard_width),
    tab_hover_tooltip: config.tab_hover_tooltip === true,
    tab_icons: normalizeTabIconConfig(config.tab_icons),
    tab_agent_indicator: normalizeTabAgentIndicator(config.tab_agent_indicator),
    panes: normalizePaneSettings(config.panes),
  }
}

function normalizePaneSettings(value: unknown): PaneSettings {
  const panes =
    typeof value === "object" && value !== null ? (value as Record<string, unknown>) : {}
  const dim = panes.dim_inactive_percent
  return {
    show_headers: panes.show_headers !== false,
    dim_inactive_percent:
      typeof dim === "number" && Number.isFinite(dim)
        ? Math.min(Math.max(Math.round(dim), 0), 60)
        : 0,
  }
}

/** A standard tab width in pixels, rounded into 80-300; 120 when unset. */
function normalizeTabStandardWidth(value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return DEFAULT_TAB_SETTINGS.tab_standard_width
  }

  return Math.min(Math.max(Math.round(value), 80), 300)
}
