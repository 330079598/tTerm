// Replay frames of a raw terminal log, as `load_terminal_log_recording`
// sends them (src-tauri/src/session_log/library.rs): a kind byte and the
// microseconds since the log started (u64), then for output a length (u32)
// and the bytes, for a size change columns and rows (u16 each). Little-endian.

const FRAME_OUTPUT = 0
const FRAME_RESIZE = 1

export type ReplayFrame =
  { atMs: number; data: Uint8Array } | { atMs: number; cols: number; rows: number }

export const DEFAULT_REPLAY_SIZE = { cols: 80, rows: 24 }

export function parseReplayFrames(buffer: ArrayBuffer): ReplayFrame[] {
  const view = new DataView(buffer)
  const bytes = new Uint8Array(buffer)
  const frames: ReplayFrame[] = []
  let offset = 0
  while (offset + 9 <= view.byteLength) {
    const kind = view.getUint8(offset)
    const atMs = Number(view.getBigUint64(offset + 1, true)) / 1000
    offset += 9
    if (kind === FRAME_OUTPUT) {
      if (offset + 4 > view.byteLength) break
      const length = view.getUint32(offset, true)
      offset += 4
      if (offset + length > view.byteLength) break
      frames.push({ atMs, data: bytes.subarray(offset, offset + length) })
      offset += length
    } else if (kind === FRAME_RESIZE) {
      if (offset + 4 > view.byteLength) break
      frames.push({
        atMs,
        cols: view.getUint16(offset, true),
        rows: view.getUint16(offset + 2, true),
      })
      offset += 4
    } else {
      break
    }
  }
  return frames
}

/** The size the recording starts at: its first known size. */
export function initialReplaySize(frames: readonly ReplayFrame[]): { cols: number; rows: number } {
  for (const frame of frames) {
    if ("cols" in frame) return { cols: frame.cols, rows: frame.rows }
  }
  return DEFAULT_REPLAY_SIZE
}

/**
 * When each frame plays, in ms from the start. With `idleLimitMs`, a pause
 * longer than that plays as that long, so a recording left open overnight
 * does not take overnight to watch.
 */
export function buildReplayTimeline(
  frames: readonly ReplayFrame[],
  idleLimitMs: number | null
): number[] {
  const timeline: number[] = []
  let previousAt = frames[0]?.atMs ?? 0
  let playAt = 0
  for (const frame of frames) {
    const gap = Math.max(0, frame.atMs - previousAt)
    playAt += idleLimitMs === null ? gap : Math.min(gap, idleLimitMs)
    previousAt = frame.atMs
    timeline.push(playAt)
  }
  return timeline
}

/** How long a timeline plays, in ms. */
export function replayDuration(timeline: readonly number[]): number {
  return timeline.length > 0 ? timeline[timeline.length - 1] : 0
}

/** How many frames have played by `timeMs`. */
export function framesPlayedBy(timeline: readonly number[], timeMs: number): number {
  let low = 0
  let high = timeline.length
  while (low < high) {
    const middle = (low + high) >> 1
    if (timeline[middle] <= timeMs) low = middle + 1
    else high = middle
  }
  return low
}

export function formatReplayTime(ms: number): string {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000))
  const hours = Math.floor(totalSeconds / 3600)
  const minutes = Math.floor((totalSeconds % 3600) / 60)
  const seconds = totalSeconds % 60
  const mmss = `${String(minutes).padStart(hours > 0 ? 2 : 1, "0")}:${String(seconds).padStart(2, "0")}`
  return hours > 0 ? `${hours}:${mmss}` : mmss
}
