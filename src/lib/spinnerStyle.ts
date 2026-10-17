import { DEFAULT_TAB_AGENT_INDICATOR, type TabAgentWorkingStyle } from "@/lib/tabAgentIndicator"

// Loading spinners take the style picked for an AI agent at work. It is kept
// here rather than read from the config context, so a spinner works in any
// dialog and test without a ConfigProvider around it.
let current: TabAgentWorkingStyle = DEFAULT_TAB_AGENT_INDICATOR.working
const listeners = new Set<() => void>()

/** The style loading spinners show. */
export function getSpinnerStyle(): TabAgentWorkingStyle {
  return current
}

/** Switches every loading spinner to `style`. */
export function setSpinnerStyle(style: TabAgentWorkingStyle) {
  if (style === current) return
  current = style
  for (const listener of listeners) listener()
}

/** Calls `listener` whenever the style changes, until the returned function is called. */
export function subscribeSpinnerStyle(listener: () => void): () => void {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}
