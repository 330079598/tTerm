/** The animations a tab can show while an AI agent works in it. */
export const TAB_AGENT_WORKING_STYLES = [
  "disc",
  "ring",
  "comet",
  "orbit",
  "pulse",
  "dots",
  "braille",
] as const
export type TabAgentWorkingStyle = (typeof TAB_AGENT_WORKING_STYLES)[number]

/**
 * The marks a tab can show while its agent waits on the user: pause bars or a
 * ring that ripple a few times, or pause bars that hold still.
 */
export const TAB_AGENT_WAITING_STYLES = ["bars", "ring", "still"] as const
export type TabAgentWaitingStyle = (typeof TAB_AGENT_WAITING_STYLES)[number]

/** How a tab shows an AI agent at work or waiting, part of the app config. */
export interface TabAgentIndicatorConfig {
  working: TabAgentWorkingStyle
  waiting: TabAgentWaitingStyle
  /** A `#rrggbb` color, or "" for the theme's primary color. */
  working_color: string
  /** A `#rrggbb` color, or "" for the theme's warning color. */
  waiting_color: string
}

export const DEFAULT_TAB_AGENT_INDICATOR: TabAgentIndicatorConfig = {
  working: "disc",
  waiting: "bars",
  working_color: "",
  waiting_color: "",
}

function pick<T extends string>(choices: readonly T[], value: unknown, fallback: T): T {
  return (choices as readonly unknown[]).includes(value) ? (value as T) : fallback
}

function normalizeColor(value: unknown): string {
  return typeof value === "string" && /^#[0-9a-f]{6}$/i.test(value) ? value.toLowerCase() : ""
}

/** A saved indicator config with unknown styles and malformed colors set back to the defaults. */
export function normalizeTabAgentIndicator(value: unknown): TabAgentIndicatorConfig {
  const saved =
    typeof value === "object" && value !== null ? (value as Record<string, unknown>) : {}
  return {
    working: pick(TAB_AGENT_WORKING_STYLES, saved.working, DEFAULT_TAB_AGENT_INDICATOR.working),
    waiting: pick(TAB_AGENT_WAITING_STYLES, saved.waiting, DEFAULT_TAB_AGENT_INDICATOR.waiting),
    working_color: normalizeColor(saved.working_color),
    waiting_color: normalizeColor(saved.waiting_color),
  }
}

/**
 * The CSS variables that tint an indicator in a chosen color: the color, and
 * the faint shade the disc and ring draw their unlit part in. None for "",
 * which leaves the theme's color.
 */
export function tabAgentColorVariables(color: string): Record<string, string> | undefined {
  if (!color) return undefined
  // 0x38 is the 22% alpha the theme's own faint shade uses.
  return { "--tab-agent-color": color, "--tab-agent-faint": `${color}38` }
}
