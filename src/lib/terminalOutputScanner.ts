import { STATUS_CONNECTING } from "@/components/TerminalTab/terminalTabUtils"

/**
 * Rolling scan of decoded terminal output. Prompt-like patterns can span
 * chunk boundaries, so only a bounded tail of the stream is kept; the per-chunk
 * cost is O(tail size) instead of O(chunk size) for full-payload regex scans.
 */
export interface TerminalOutputScanState {
  tail: string
}

export interface TerminalOutputScanResult {
  state: TerminalOutputScanState
  /** The sudo prompt username, when a sudo password prompt was detected. */
  sudoPromptUser: string | null
  /** True when a cold-path "[Connecting" status line was seen. */
  connecting: boolean
}

const SUDO_PASSWORD_PROMPT = /^\[sudo\] password for ([^:]+?):?[ \t]*$/
/** A sudo prompt is at most ~200 bytes and always arrives as one line tail. */
const MAX_TAIL_CHARS = 512

export const EMPTY_OUTPUT_SCAN_STATE: TerminalOutputScanState = { tail: "" }

export function scanTerminalOutput(
  current: TerminalOutputScanState,
  chunk: string
): TerminalOutputScanResult {
  const tail = current.tail ? current.tail + chunk : chunk
  const boundedTail = tail.length > MAX_TAIL_CHARS ? tail.slice(tail.length - MAX_TAIL_CHARS) : tail

  const state = { tail: boundedTail }
  // Only the final line can be an active prompt; CR, LF, and CRLF all end it.
  const lastLine = boundedTail.slice(boundedTail.lastIndexOf("\n") + 1)
  const effectiveLine = lastLine.slice(lastLine.lastIndexOf("\r") + 1)
  const match = effectiveLine.match(SUDO_PASSWORD_PROMPT)
  const connecting = boundedTail.includes(STATUS_CONNECTING)

  return {
    state,
    sudoPromptUser: match ? match[1].trim() : null,
    connecting,
  }
}

/**
 * Decodes a binary channel payload into text. The backend batches on UTF-8
 * sequence boundaries, but the stream can still carry malformed bytes (remote
 * non-UTF-8 locale output, `cat` on a binary); those decode lossily to U+FFFD
 * instead of dropping the whole chunk.
 */
export function decodeOutputChunk(payload: ArrayBuffer | Uint8Array): string {
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(payload)
  } catch {
    return new TextDecoder("utf-8").decode(payload)
  }
}
