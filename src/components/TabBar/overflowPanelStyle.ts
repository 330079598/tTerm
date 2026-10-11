import type React from "react"

const OVERFLOW_PANEL_MAX_WIDTH = 320
const OVERFLOW_PANEL_VIEWPORT_RATIO = 0.7
const OVERFLOW_PANEL_MARGIN = 8

export type OverflowPanelStyle = React.CSSProperties & {
  "--tab-overflow-panel-max-height"?: string
}

/**
 * Where the overflow menu's panel opens: under its trigger, right-aligned to
 * it, and kept inside a window `viewportWidth` by `viewportHeight`.
 */
export function placeOverflowPanel(
  triggerRect: Pick<DOMRect, "right" | "bottom">,
  viewportWidth: number,
  viewportHeight: number
): OverflowPanelStyle {
  const viewportMargin = Math.min(OVERFLOW_PANEL_MARGIN, viewportWidth / 2)
  const availableWidth = Math.max(0, viewportWidth - viewportMargin * 2)
  const panelWidth = Math.min(
    OVERFLOW_PANEL_MAX_WIDTH,
    viewportWidth * OVERFLOW_PANEL_VIEWPORT_RATIO,
    availableWidth
  )
  const left = Math.min(
    Math.max(viewportMargin, triggerRect.right - panelWidth),
    viewportWidth - panelWidth - viewportMargin
  )
  const top = triggerRect.bottom + 4
  const maxHeight = Math.max(120, Math.min(420, viewportHeight - top - viewportMargin))

  return {
    left,
    top,
    width: panelWidth,
    maxHeight,
    "--tab-overflow-panel-max-height": `${maxHeight}px`,
  }
}
