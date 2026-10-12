import type { Terminal } from "@xterm/xterm"

import type { ReplayFrame } from "@/lib/terminalLogReplay"

/** Output written to the terminal in one go while seeking. */
const WRITE_CHUNK_BYTES = 512 * 1024
function concatBytes(chunks: Uint8Array[]): Uint8Array {
  if (chunks.length === 1) return chunks[0]
  const total = chunks.reduce((sum, chunk) => sum + chunk.length, 0)
  const joined = new Uint8Array(total)
  let offset = 0
  for (const chunk of chunks) {
    joined.set(chunk, offset)
    offset += chunk.length
  }
  return joined
}

/**
 * Writes frames `from..to` in order. A size change waits for the output
 * before it, since the terminal parses writes later than it resizes. Stops
 * early, returning false, once `alive` says the write is no longer wanted.
 */
export async function writeFrames(
  term: Terminal,
  frames: readonly ReplayFrame[],
  from: number,
  to: number,
  alive: () => boolean
): Promise<boolean> {
  let pending: Uint8Array[] = []
  let pendingBytes = 0
  const flush = (after?: () => void) =>
    new Promise<void>((resolve) => {
      const data = pending.length > 0 ? concatBytes(pending) : new Uint8Array()
      pending = []
      pendingBytes = 0
      term.write(data, () => {
        if (alive()) after?.()
        resolve()
      })
    })

  for (let index = from; index < to; index++) {
    const frame = frames[index]
    if ("data" in frame) {
      pending.push(frame.data)
      pendingBytes += frame.data.length
      if (pendingBytes >= WRITE_CHUNK_BYTES) {
        await flush()
        if (!alive()) return false
      }
    } else {
      const { cols, rows } = frame
      await flush(() => term.resize(cols, rows))
      if (!alive()) return false
    }
  }
  if (pending.length > 0) await flush()
  return alive()
}
