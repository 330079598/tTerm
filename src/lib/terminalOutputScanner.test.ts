import { describe, expect, it } from "vitest"

import {
  decodeOutputChunk,
  EMPTY_OUTPUT_SCAN_STATE,
  scanTerminalOutput,
} from "@/lib/terminalOutputScanner"

describe("scanTerminalOutput", () => {
  it("keeps only a marker-sized carry under a flood", () => {
    let state = EMPTY_OUTPUT_SCAN_STATE
    for (let round = 0; round < 50; round += 1) {
      state = scanTerminalOutput(state, "x".repeat(1000)).state
    }
    expect(state.tail.length).toBeLessThan("[Connecting".length)
  })

  it("detects cold-path connecting status lines", () => {
    const result = scanTerminalOutput(
      EMPTY_OUTPUT_SCAN_STATE,
      "\r\n\x1b[33m[Connecting to jump host #1 user@host:22]\x1b[0m\r\n"
    )
    expect(result.connecting).toBe(true)
  })

  it("does not flag a connecting marker left over in the rolling tail", () => {
    const first = scanTerminalOutput(
      EMPTY_OUTPUT_SCAN_STATE,
      "\r\n\x1b[33m[Connecting to jump host #1 user@host:22]\x1b[0m\r\n"
    )
    // Still well inside the 512-char tail, but the live chunk is ordinary
    // output: echoed commands or `grep` hits must not re-trigger connecting.
    const second = scanTerminalOutput(first.state, "$ grep Connect logs\n")
    expect(second.connecting).toBe(false)
  })

  it("detects a connecting marker split across chunk boundaries", () => {
    const first = scanTerminalOutput(EMPTY_OUTPUT_SCAN_STATE, "\r\n\x1b[33m[Conn")
    expect(first.connecting).toBe(false)
    const second = scanTerminalOutput(first.state, "ecting to user@host:22]\x1b[0m\r\n")
    expect(second.connecting).toBe(true)
  })

  it("reports connecting false for ordinary output", () => {
    const result = scanTerminalOutput(EMPTY_OUTPUT_SCAN_STATE, "$ echo hi\nhi\n")
    expect(result.connecting).toBe(false)
  })
})

describe("decodeOutputChunk", () => {
  it("decodes valid utf-8 bytes", () => {
    expect(decodeOutputChunk(new TextEncoder().encode("héllo 世界"))).toBe("héllo 世界")
  })

  it("falls back to replacement characters for malformed bytes", () => {
    expect(decodeOutputChunk(new Uint8Array([0xe4, 0xb8]))).toBe("\uFFFD")
  })

  it("keeps valid bytes around a malformed sequence", () => {
    expect(decodeOutputChunk(new Uint8Array([0x61, 0xe4, 0xb8, 0x62]))).toBe("a\uFFFDb")
  })

  it("decodes control characters and escape sequences", () => {
    const payload = new TextEncoder().encode("\r\n\x1b[31mred\x1b[0m")
    expect(decodeOutputChunk(payload)).toBe("\r\n\x1b[31mred\x1b[0m")
  })
})
