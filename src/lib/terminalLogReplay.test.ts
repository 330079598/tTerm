import { describe, expect, it } from "vitest"

import {
  buildReplayTimeline,
  formatReplayTime,
  framesPlayedBy,
  initialReplaySize,
  parseReplayFrames,
  type ReplayFrame,
} from "@/lib/terminalLogReplay"

function encode(frames: Array<{ atUs: number; data?: string; cols?: number; rows?: number }>) {
  const parts: number[] = []
  const push64 = (value: number) => {
    const view = new DataView(new ArrayBuffer(8))
    view.setBigUint64(0, BigInt(value), true)
    parts.push(...new Uint8Array(view.buffer))
  }
  for (const frame of frames) {
    if (frame.data !== undefined) {
      const bytes = new TextEncoder().encode(frame.data)
      parts.push(0)
      push64(frame.atUs)
      const length = new DataView(new ArrayBuffer(4))
      length.setUint32(0, bytes.length, true)
      parts.push(...new Uint8Array(length.buffer), ...bytes)
    } else {
      parts.push(1)
      push64(frame.atUs)
      const size = new DataView(new ArrayBuffer(4))
      size.setUint16(0, frame.cols ?? 0, true)
      size.setUint16(2, frame.rows ?? 0, true)
      parts.push(...new Uint8Array(size.buffer))
    }
  }
  return new Uint8Array(parts).buffer
}

describe("terminalLogReplay", () => {
  it("parses output and size frames", () => {
    const frames = parseReplayFrames(
      encode([
        { atUs: 0, cols: 100, rows: 30 },
        { atUs: 1500, data: "hi" },
      ])
    )
    expect(frames).toHaveLength(2)
    expect(frames[0]).toEqual({ atMs: 0, cols: 100, rows: 30 })
    const output = frames[1] as { atMs: number; data: Uint8Array }
    expect(output.atMs).toBe(1.5)
    expect(new TextDecoder().decode(output.data)).toBe("hi")
    expect(initialReplaySize(frames)).toEqual({ cols: 100, rows: 30 })
  })

  it("stops at a truncated frame", () => {
    const buffer = encode([{ atUs: 0, data: "hello" }])
    expect(parseReplayFrames(buffer.slice(0, buffer.byteLength - 2))).toEqual([])
  })

  it("shortens long pauses when asked", () => {
    const frames: ReplayFrame[] = [
      { atMs: 1000, data: new Uint8Array() },
      { atMs: 1200, data: new Uint8Array() },
      { atMs: 61_200, data: new Uint8Array() },
    ]
    expect(buildReplayTimeline(frames, null)).toEqual([0, 200, 60_200])
    expect(buildReplayTimeline(frames, 2000)).toEqual([0, 200, 2200])
  })

  it("counts the frames played by a time", () => {
    const timeline = [0, 200, 2200]
    expect(framesPlayedBy(timeline, -1)).toBe(0)
    expect(framesPlayedBy(timeline, 0)).toBe(1)
    expect(framesPlayedBy(timeline, 1000)).toBe(2)
    expect(framesPlayedBy(timeline, 5000)).toBe(3)
  })

  it("formats playback time", () => {
    expect(formatReplayTime(0)).toBe("0:00")
    expect(formatReplayTime(65_000)).toBe("1:05")
    expect(formatReplayTime(3_725_000)).toBe("1:02:05")
  })
})
