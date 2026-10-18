import type {
  SerialFlowControl,
  SerialLineEnding,
  SerialParity,
  SerialPortEntry,
  SerialSettings,
  SerialUsbId,
} from "@/types/serial"

/** The baud rates the connection dialog offers; any other can be typed. */
export const SERIAL_BAUD_RATES = [
  300, 1200, 2400, 4800, 9600, 14400, 19200, 38400, 57600, 74880, 115200, 230400, 250000, 460800,
  500000, 921600, 1000000, 1500000, 2000000, 3000000,
] as const

export const SERIAL_DATA_BITS = [5, 6, 7, 8] as const
export const SERIAL_STOP_BITS = [1, 2] as const
export const SERIAL_PARITIES: readonly SerialParity[] = ["none", "odd", "even"]
export const SERIAL_FLOW_CONTROLS: readonly SerialFlowControl[] = ["none", "software", "hardware"]
export const SERIAL_LINE_ENDINGS: readonly SerialLineEnding[] = ["cr", "lf", "crlf"]

export const DEFAULT_SERIAL_SETTINGS: SerialSettings = {
  port: "",
  baudRate: 115200,
  dataBits: 8,
  parity: "none",
  stopBits: 1,
  flowControl: "none",
  lineEnding: "cr",
  localEcho: false,
  timestamps: false,
}

function oneOf<T>(choices: readonly T[], value: unknown, fallback: T): T {
  return choices.includes(value as T) ? (value as T) : fallback
}

function normalizeUsbId(value: unknown): SerialUsbId | undefined {
  if (!value || typeof value !== "object") return undefined
  const { vid, pid, serialNumber } = value as Record<string, unknown>
  if (!Number.isInteger(vid) || !Number.isInteger(pid)) return undefined
  return {
    vid: vid as number,
    pid: pid as number,
    serialNumber: typeof serialNumber === "string" && serialNumber ? serialNumber : undefined,
  }
}

/** Saved or typed settings with anything a port can't open with replaced by its default. */
export function normalizeSerialSettings(value: unknown): SerialSettings {
  const raw = value && typeof value === "object" ? (value as Record<string, unknown>) : {}
  const baudRate = Number(raw.baudRate)
  return {
    port: typeof raw.port === "string" ? raw.port.trim() : "",
    baudRate:
      Number.isInteger(baudRate) && baudRate > 0 ? baudRate : DEFAULT_SERIAL_SETTINGS.baudRate,
    dataBits: oneOf(SERIAL_DATA_BITS, raw.dataBits, 8),
    parity: oneOf(SERIAL_PARITIES, raw.parity, "none"),
    stopBits: oneOf(SERIAL_STOP_BITS, raw.stopBits, 1),
    flowControl: oneOf(SERIAL_FLOW_CONTROLS, raw.flowControl, "none"),
    lineEnding: oneOf(SERIAL_LINE_ENDINGS, raw.lineEnding, "cr"),
    localEcho: raw.localEcho === true,
    timestamps: raw.timestamps === true,
    usb: normalizeUsbId(raw.usb),
  }
}

/** The framing in the usual shorthand: `8N1`, `7E2`. */
export function serialFraming(settings: SerialSettings): string {
  return `${settings.dataBits}${settings.parity[0].toUpperCase()}${settings.stopBits}`
}

/** `115200 8N1`. */
export function serialSummary(settings: SerialSettings): string {
  return `${settings.baudRate} ${serialFraming(settings)}`
}

/** `1A86:7523`, as device managers show a USB device. */
export function formatUsbId(usb: SerialUsbId): string {
  const hex = (value: number) => value.toString(16).toUpperCase().padStart(4, "0")
  return `${hex(usb.vid)}:${hex(usb.pid)}`
}

/** What a port is, for the dialog's list: its product, else its maker. */
export function serialPortDescription(entry: SerialPortEntry): string | undefined {
  return entry.product?.trim() || entry.manufacturer?.trim() || undefined
}

/** Settings pointed at `entry`, remembering its USB device so it is found after a replug. */
export function withSerialPort(settings: SerialSettings, entry: SerialPortEntry): SerialSettings {
  return { ...settings, port: entry.path, usb: entry.usb }
}

/**
 * What the terminal shows for typed text when the device doesn't echo it:
 * Enter starts a new line and Backspace rubs out a character. Replies the
 * terminal sends itself, which start with ESC, show nothing.
 */
export function localEchoText(data: string): string {
  if (data.startsWith("\x1b")) return ""
  return data
    .split("\r\n")
    .join("\r")
    .split("")
    .map((char) => (char === "\r" ? "\r\n" : char === "\x7f" || char === "\b" ? "\b \b" : char))
    .join("")
}

function formatClock(now: Date): string {
  const pad = (value: number, width = 2) => String(value).padStart(width, "0")
  return `${pad(now.getHours())}:${pad(now.getMinutes())}:${pad(now.getSeconds())}.${pad(now.getMilliseconds(), 3)}`
}

/**
 * Starts each line of output with the time it arrived, dimmed. A line split
 * across chunks gets one stamp, from the chunk that started it.
 */
export function createLineTimestamper() {
  let atLineStart = true
  return (text: string, now: Date = new Date()): string => {
    const stamp = `\x1b[2m[${formatClock(now)}]\x1b[22m `
    let stamped = ""
    for (const char of text) {
      if (atLineStart && char !== "\r" && char !== "\n") {
        stamped += stamp
        atLineStart = false
      }
      stamped += char
      if (char === "\n") atLineStart = true
    }
    return stamped
  }
}
