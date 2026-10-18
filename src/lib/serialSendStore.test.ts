// @vitest-environment jsdom

import { beforeEach, describe, expect, it } from "vitest"

import {
  DEFAULT_SERIAL_SEND_MEMORY,
  loadSerialSendMemory,
  saveSerialSendMemory,
  SERIAL_SEND_STORAGE_KEY,
  serialSendKey,
  withSentEntry,
} from "@/lib/serialSendStore"

describe("serial send memory", () => {
  beforeEach(() => localStorage.clear())

  it("keeps a connection's history and echo choice", () => {
    const key = serialSendKey("board", "COM3")
    const memory = { history: [{ mode: "hex" as const, text: "AA 55" }], echo: false }
    saveSerialSendMemory(key, memory)
    expect(loadSerialSendMemory(key)).toEqual(memory)
    expect(loadSerialSendMemory(serialSendKey(undefined, "COM3"))).toEqual(
      DEFAULT_SERIAL_SEND_MEMORY
    )
  })

  it("ignores what it can't read", () => {
    localStorage.setItem(SERIAL_SEND_STORAGE_KEY, "not json")
    expect(loadSerialSendMemory("port:COM1")).toEqual(DEFAULT_SERIAL_SEND_MEMORY)
    localStorage.setItem(
      SERIAL_SEND_STORAGE_KEY,
      JSON.stringify({
        "port:COM1": {
          history: [
            { mode: "bin", text: 1 },
            { mode: "text", text: "ok" },
          ],
        },
      })
    )
    expect(loadSerialSendMemory("port:COM1")).toEqual({
      history: [{ mode: "text", text: "ok" }],
      echo: true,
    })
  })

  it("forgets the least recently used connection first", () => {
    for (let index = 0; index < 51; index += 1) {
      saveSerialSendMemory(`port:COM${index}`, DEFAULT_SERIAL_SEND_MEMORY)
    }
    const stored = JSON.parse(localStorage.getItem(SERIAL_SEND_STORAGE_KEY) ?? "{}")
    expect(Object.keys(stored)).toHaveLength(50)
    expect(stored["port:COM0"]).toBeUndefined()
    expect(stored["port:COM50"]).toBeDefined()
  })

  it("puts what was sent first, once", () => {
    const history = [
      { mode: "text" as const, text: "a" },
      { mode: "text" as const, text: "b" },
    ]
    expect(withSentEntry(history, { mode: "text", text: "b" })).toEqual([
      { mode: "text", text: "b" },
      { mode: "text", text: "a" },
    ])
  })
})
