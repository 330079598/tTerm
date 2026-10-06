import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import {
  OUTPUT_ACK_BATCH_BYTES,
  OUTPUT_ACK_DELAY_MS,
  OutputAcker,
} from "@/components/TerminalTab/outputAck"

describe("OutputAcker", () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it("acknowledges a full batch immediately", () => {
    const send = vi.fn()
    const acker = new OutputAcker(send)
    acker.parsed(OUTPUT_ACK_BATCH_BYTES - 1)
    expect(send).not.toHaveBeenCalled()
    acker.parsed(1)
    expect(send).toHaveBeenCalledWith(OUTPUT_ACK_BATCH_BYTES)
  })

  it("acknowledges small remainders after a short delay, coalesced", () => {
    const send = vi.fn()
    const acker = new OutputAcker(send)
    acker.parsed(10)
    acker.parsed(20)
    expect(send).not.toHaveBeenCalled()
    vi.advanceTimersByTime(OUTPUT_ACK_DELAY_MS)
    expect(send).toHaveBeenCalledTimes(1)
    expect(send).toHaveBeenCalledWith(30)
  })

  it("does not send a remainder already flushed by a full batch", () => {
    const send = vi.fn()
    const acker = new OutputAcker(send)
    acker.parsed(10)
    acker.parsed(OUTPUT_ACK_BATCH_BYTES)
    vi.advanceTimersByTime(OUTPUT_ACK_DELAY_MS)
    expect(send).toHaveBeenCalledTimes(1)
    expect(send).toHaveBeenCalledWith(OUTPUT_ACK_BATCH_BYTES + 10)
  })

  it("sends nothing after dispose", () => {
    const send = vi.fn()
    const acker = new OutputAcker(send)
    acker.parsed(10)
    acker.dispose()
    acker.parsed(OUTPUT_ACK_BATCH_BYTES)
    vi.advanceTimersByTime(OUTPUT_ACK_DELAY_MS)
    expect(send).not.toHaveBeenCalled()
  })
})
