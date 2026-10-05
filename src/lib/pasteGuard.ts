/** Lines of a guarded paste shown in the confirmation. */
export const PASTE_PREVIEW_MAX_LINES = 8
/** Characters kept of each previewed line. */
export const PASTE_PREVIEW_MAX_LINE_LENGTH = 160

/**
 * Whether pasting `text` should be confirmed first. A line break runs the
 * line before it, unless the receiving program turned on bracketed paste
 * mode and so holds the whole paste until Enter is pressed.
 */
export function pasteNeedsConfirmation(
  text: string,
  options: { enabled: boolean; bracketedPasteMode: boolean }
): boolean {
  return options.enabled && !options.bracketedPasteMode && /[\r\n]/.test(text)
}

/** The lines of a paste and the start of it to show before it is sent. */
export function summarizePaste(text: string): { lineCount: number; preview: string } {
  const lines = text.replace(/\r\n?/g, "\n").split("\n")
  // A trailing line break ends the last line rather than starting another.
  if (lines.length > 1 && lines[lines.length - 1] === "") lines.pop()

  const shown = lines
    .slice(0, PASTE_PREVIEW_MAX_LINES)
    .map((line) =>
      line.length > PASTE_PREVIEW_MAX_LINE_LENGTH
        ? `${line.slice(0, PASTE_PREVIEW_MAX_LINE_LENGTH)}…`
        : line
    )
  if (lines.length > PASTE_PREVIEW_MAX_LINES) shown.push("…")
  return { lineCount: lines.length, preview: shown.join("\n") }
}
