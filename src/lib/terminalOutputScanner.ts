import { STATUS_CONNECTING } from "@/components/TerminalTab/terminalTabUtils"

/**
 * Rolling scan of decoded terminal output for the cold-path "[Connecting"
 * status line. The marker can span chunk boundaries, so a carry of
 * marker-length-minus-one chars is kept between chunks.
 */
export interface TerminalOutputScanState {
  tail: string
}

export interface TerminalOutputScanResult {
  state: TerminalOutputScanState
  /** True when a cold-path "[Connecting" status line was seen in this chunk. */
  connecting: boolean
}

const CONNECTING_CARRY_CHARS = STATUS_CONNECTING.length - 1

export const EMPTY_OUTPUT_SCAN_STATE: TerminalOutputScanState = { tail: "" }

export function scanTerminalOutput(
  current: TerminalOutputScanState,
  chunk: string
): TerminalOutputScanResult {
  // The marker must reflect the live stream: only the carry joins the chunk,
  // so historical output that merely contains the marker (echoed commands,
  // `grep` hits) cannot flag it again.
  const window = current.tail + chunk
  return {
    state: { tail: window.slice(-CONNECTING_CARRY_CHARS) },
    connecting: window.includes(STATUS_CONNECTING),
  }
}

/**
 * Decodes a binary channel payload into text. The backend batches on UTF-8
 * sequence boundaries, but the stream can still carry malformed bytes (remote
 * non-UTF-8 locale output, `cat` on a binary); those decode lossily to U+FFFD
 * instead of dropping the whole chunk. The shared decoder is safe because
 * decode() without `{ stream: true }` leaves no state between calls.
 */
const chunkDecoder = new TextDecoder("utf-8")

export function decodeOutputChunk(payload: ArrayBuffer | Uint8Array): string {
  return chunkDecoder.decode(payload)
}
