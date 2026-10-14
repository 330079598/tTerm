// The count of unseen notifications as Windows shows it: an overlay on the
// taskbar button, drawn here because Windows only takes a picture there.

/** `OverlayImage` in src-tauri/src/notification/badge.rs. */
export interface BadgeOverlay {
  rgba: number[]
  width: number
  height: number
}

/** The overlay's red, the same as the dot on the tray icon. */
const BADGE_COLOR = "#E81123"

/** One digit fits a 16px overlay; two would be too small to read. */
export function badgeLabel(count: number): string {
  return count > 9 ? "9+" : String(count)
}

/** Windows shows the overlay at 16px scaled with the display. */
export function badgeOverlaySize(devicePixelRatio: number): number {
  return Math.max(16, Math.round(16 * devicePixelRatio))
}

/** A red disc with the count in white, or null where there is no canvas. */
export function drawBadgeOverlay(count: number, size: number): BadgeOverlay | null {
  if (typeof OffscreenCanvas === "undefined") return null
  const context = new OffscreenCanvas(size, size).getContext("2d")
  if (!context) return null

  context.fillStyle = BADGE_COLOR
  context.beginPath()
  context.arc(size / 2, size / 2, size / 2, 0, Math.PI * 2)
  context.fill()

  const label = badgeLabel(count)
  const fontSize = Math.round(size * (label.length > 1 ? 0.56 : 0.72))
  context.font = `bold ${fontSize}px "Segoe UI", sans-serif`
  context.fillStyle = "#FFFFFF"
  context.textAlign = "center"
  context.textBaseline = "alphabetic"
  // Center the glyphs themselves; the font's line box sits them too high.
  const metrics = context.measureText(label)
  const baseline =
    size / 2 + (metrics.actualBoundingBoxAscent - metrics.actualBoundingBoxDescent) / 2
  context.fillText(label, size / 2, baseline)

  return {
    rgba: Array.from(context.getImageData(0, 0, size, size).data),
    width: size,
    height: size,
  }
}
