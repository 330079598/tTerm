import { describe, expect, it } from "vitest"

import {
  createLineTimestamper,
  DEFAULT_SERIAL_SETTINGS,
  formatUsbId,
  lineEndingBytes,
  localEchoText,
  normalizeSerialSettings,
  parseHexBytes,
  serialPortDescription,
  serialSummary,
  withSerialPort,
} from "@/lib/serial"

describe("normalizeSerialSettings", () => {
  it("fills in 115200 8N1 for what is missing", () => {
    expect(normalizeSerialSettings({ port: " COM3 " })).toEqual({
      ...DEFAULT_SERIAL_SETTINGS,
      port: "COM3",
    })
  })

  it("replaces what no port opens with", () => {
    const settings = normalizeSerialSettings({
      port: "COM3",
      baudRate: -5,
      dataBits: 9,
      parity: "mark",
      stopBits: 3,
      flowControl: "dsr",
      lineEnding: "lfcr",
      usb: { vid: "x", pid: 1 },
    })
    expect(settings).toEqual({ ...DEFAULT_SERIAL_SETTINGS, port: "COM3" })
  })

  it("keeps a saved device", () => {
    const settings = normalizeSerialSettings({
      port: "/dev/ttyUSB0",
      baudRate: 921600,
      parity: "even",
      usb: { vid: 0x1a86, pid: 0x7523, serialNumber: "" },
    })
    expect(settings.baudRate).toBe(921600)
    expect(settings.usb).toEqual({ vid: 0x1a86, pid: 0x7523, serialNumber: undefined })
    expect(serialSummary(settings)).toBe("921600 8E1")
  })
})

describe("ports", () => {
  it("shows a USB device as device managers do", () => {
    expect(formatUsbId({ vid: 0x1a86, pid: 0x7523 })).toBe("1A86:7523")
  })

  it("describes a port by its product, else its maker", () => {
    expect(serialPortDescription({ path: "COM3", kind: "usb", product: "CH340" })).toBe("CH340")
    expect(serialPortDescription({ path: "COM3", kind: "usb", manufacturer: "FTDI" })).toBe("FTDI")
    expect(serialPortDescription({ path: "COM1", kind: "pci" })).toBeUndefined()
  })

  it("remembers the device of a chosen port, and forgets it for one without", () => {
    const usb = { vid: 1, pid: 2, serialNumber: "A" }
    const chosen = withSerialPort(DEFAULT_SERIAL_SETTINGS, { path: "COM4", kind: "usb", usb })
    expect(chosen).toMatchObject({ port: "COM4", usb })
    expect(withSerialPort(chosen, { path: "COM1", kind: "pci" }).usb).toBeUndefined()
  })
})

describe("localEchoText", () => {
  it("starts a new line on Enter and rubs out on Backspace", () => {
    expect(localEchoText("ls\r")).toBe("ls\r\n")
    expect(localEchoText("a\r\nb")).toBe("a\r\nb")
    expect(localEchoText("\x7f")).toBe("\b \b")
  })

  it("shows nothing for the terminal's own replies", () => {
    expect(localEchoText("\x1b[I")).toBe("")
  })
})

describe("createLineTimestamper", () => {
  const now = new Date(2026, 0, 1, 9, 5, 7, 42)
  const stamp = "\x1b[2m[09:05:07.042]\x1b[22m "

  it("stamps each line once, even split across chunks", () => {
    const stampLines = createLineTimestamper()
    expect(stampLines("boot", now)).toBe(`${stamp}boot`)
    expect(stampLines("ing\r\nready\r\n", now)).toBe(`ing\r\n${stamp}ready\r\n`)
    expect(stampLines("> ", now)).toBe(`${stamp}> `)
  })

  it("leaves empty lines alone", () => {
    expect(createLineTimestamper()("\r\n\r\n", now)).toBe("\r\n\r\n")
  })
})

describe("parseHexBytes", () => {
  it("reads hex the ways people write it", () => {
    const expected = { ok: true, bytes: [0x48, 0x65, 0x6c] }
    expect(parseHexBytes("48 65 6C")).toEqual(expected)
    expect(parseHexBytes("48,65,6c")).toEqual(expected)
    expect(parseHexBytes("0x48 0x65 0x6C")).toEqual(expected)
    expect(parseHexBytes(" 48656c ")).toEqual(expected)
    expect(parseHexBytes("")).toEqual({ ok: true, bytes: [] })
  })

  it("names the token that isn't hex", () => {
    expect(parseHexBytes("48 6G")).toEqual({ ok: false, token: "6G" })
    expect(parseHexBytes("48 656")).toEqual({ ok: false, token: "656" })
  })
})

describe("lineEndingBytes", () => {
  it("sends what Enter sends", () => {
    expect(lineEndingBytes("cr")).toEqual([0x0d])
    expect(lineEndingBytes("lf")).toEqual([0x0a])
    expect(lineEndingBytes("crlf")).toEqual([0x0d, 0x0a])
  })
})
