/**
 * How many characters a password prompt holds after `data` is typed, counted
 * the way a no-echo line reader edits its line; null once Enter or Ctrl+C
 * ends the prompt.
 */
export function nextTypedLength(typed: number, data: string): number | null {
  let length = typed
  for (const char of data) {
    switch (char) {
      case "\r":
      case "\n":
      case "\x03":
        return null
      // Backspace, as DEL or as Ctrl+H.
      case "\x7f":
      case "\b":
        length = Math.max(0, length - 1)
        break
      // Ctrl+U kills the line; sudo and getpass both honor it.
      case "\x15":
        length = 0
        break
      default:
        // Other control characters are not part of the password.
        if (char >= " ") length += 1
    }
  }
  return length
}
