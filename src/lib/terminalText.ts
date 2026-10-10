import type { IBuffer } from "@xterm/xterm"

/**
 * The text of a terminal buffer, scrollback included: rows the terminal
 * wrapped are joined back into their line, trailing blanks and trailing
 * empty lines dropped.
 */
export function terminalBufferText(buffer: IBuffer): string {
  const lines: string[] = []
  let logical = ""
  for (let index = 0; index < buffer.length; index++) {
    const line = buffer.getLine(index)
    if (!line) continue
    const continues = buffer.getLine(index + 1)?.isWrapped === true
    // Blanks at a wrap are part of the line; only its end is trimmed.
    logical += line.translateToString(!continues)
    if (!continues) {
      lines.push(logical)
      logical = ""
    }
  }
  if (logical) lines.push(logical)
  while (lines.length > 0 && lines[lines.length - 1] === "") lines.pop()
  return lines.length > 0 ? `${lines.join("\n")}\n` : ""
}

/** A file name for saved terminal text: the tab's name and the time. */
export function terminalTextFileName(name: string, now: Date): string {
  const safe =
    Array.from(name, (character) =>
      character < " " || '<>:"/\\|?*'.includes(character) ? "_" : character
    )
      .join("")
      .trim() || "terminal"
  const pad = (value: number) => String(value).padStart(2, "0")
  const stamp = `${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}-${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`
  return `${safe}-${stamp}.txt`
}
