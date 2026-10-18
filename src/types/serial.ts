export type SerialParity = "none" | "odd" | "even"

/** `software` is XON/XOFF, `hardware` is RTS/CTS. */
export type SerialFlowControl = "none" | "software" | "hardware"

/** What the Enter key sends; xterm.js types a carriage return. */
export type SerialLineEnding = "cr" | "lf" | "crlf"

/**
 * Which lone byte of a received line break also gets the other half, for
 * devices that end lines, or echo Enter, with only a CR or an LF.
 */
export type SerialReceiveNewline = "none" | "cr" | "lf"

/** The USB device behind a port, which finds it again under another name. */
export interface SerialUsbId {
  vid: number
  pid: number
  serialNumber?: string
}

/** A serial connection's settings, as a tab and a saved profile carry them. */
export interface SerialSettings {
  /** `COM3`, `/dev/ttyUSB0`, `/dev/cu.usbserial-1410`. */
  port: string
  baudRate: number
  /** 5 to 8. */
  dataBits: number
  parity: SerialParity
  /** 1 or 2. */
  stopBits: number
  flowControl: SerialFlowControl
  lineEnding: SerialLineEnding
  receiveNewline: SerialReceiveNewline
  /** Show typed text in the terminal, for devices that don't echo it. */
  localEcho: boolean
  /** Start each line of output with the time it arrived. */
  timestamps: boolean
  /** Show received bytes in hex instead of as text. */
  hexView: boolean
  usb?: SerialUsbId
}

/** One port this machine has, from `list_serial_ports`. */
export interface SerialPortEntry {
  path: string
  kind: "usb" | "pci" | "bluetooth" | "unknown"
  usb?: SerialUsbId
  manufacturer?: string
  product?: string
}
