import { decodeBase64Utf8 } from "@/lib/terminalNotifications"

/** The OSC that programs use to set the clipboard (`ESC ] 52 ; Pc ; Pd BEL`). */
export const OSC52_CODE = 52

/**
 * Returns the text an OSC 52 sequence asks to copy, given the sequence's data
 * (`Pc;Pd`), or null when it does not ask to copy text.
 *
 * Every selection target in `Pc` goes to the system clipboard, as iTerm2 and
 * WezTerm do, since a desktop has no separate primary selection worth
 * keeping. A query (`Pd` of `?`) returns null: answering it would let any
 * program, including one on a remote host, read the local clipboard. So does
 * an empty `Pd`, which asks to clear the clipboard, since clearing it is
 * never what the user wants from a copy.
 */
export function parseOsc52Copy(data: string): string | null {
  const separator = data.indexOf(";")
  if (separator < 0) return null
  const targets = data.slice(0, separator)
  if (!/^[cpqs0-7]*$/.test(targets)) return null
  const payload = data.slice(separator + 1)
  if (payload === "" || payload === "?") return null
  const text = decodeBase64Utf8(payload)
  return text ? text : null
}
