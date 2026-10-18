// @vitest-environment jsdom

import { beforeEach, describe, expect, it } from "vitest"

import {
  loadSerialSendEcho,
  saveSerialSendEcho,
  SERIAL_SEND_ECHO_STORAGE_KEY,
  serialSendKey,
} from "@/lib/serialSendStore"

describe("serial send echo choice", () => {
  beforeEach(() => localStorage.clear())

  it("keys saved connections by profile and others by port", () => {
    expect(serialSendKey("board", "COM3")).toBe("profile:board")
    expect(serialSendKey(undefined, "COM3")).toBe("port:COM3")
  })

  it("shows by default and remembers a connection that hides", () => {
    const key = serialSendKey("board", "COM3")
    expect(loadSerialSendEcho(key)).toBe(true)
    saveSerialSendEcho(key, false)
    expect(loadSerialSendEcho(key)).toBe(false)
    expect(loadSerialSendEcho("port:COM3")).toBe(true)
    saveSerialSendEcho(key, true)
    expect(loadSerialSendEcho(key)).toBe(true)
  })

  it("ignores what it can't read", () => {
    localStorage.setItem(SERIAL_SEND_ECHO_STORAGE_KEY, "not json")
    expect(loadSerialSendEcho("port:COM1")).toBe(true)
    localStorage.setItem(SERIAL_SEND_ECHO_STORAGE_KEY, JSON.stringify([1, "port:COM1"]))
    expect(loadSerialSendEcho("port:COM1")).toBe(false)
  })

  it("forgets the least recently hidden connection first", () => {
    for (let index = 0; index < 51; index += 1) {
      saveSerialSendEcho(`port:COM${index}`, false)
    }
    expect(loadSerialSendEcho("port:COM0")).toBe(true)
    expect(loadSerialSendEcho("port:COM50")).toBe(false)
  })
})
