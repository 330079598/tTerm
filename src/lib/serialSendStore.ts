/** How the send bar sends: text the way typing does, or hex bytes as they are. */
export type SerialSendMode = "text" | "hex"

/** One thing to send from the send bar. */
export interface SerialSendEntry {
  mode: SerialSendMode
  text: string
}

/** One thing a connection's send bar sent, as the database keeps it. */
export interface SerialSend extends SerialSendEntry {
  id: string
  connectionKey: string
  /** Unix milliseconds of the last time it was sent. */
  sentAt: number
}

/**
 * Connections that hide what their send bar sends. Showing it is the
 * default, and a per-device view choice, so it stays in this browser while
 * the history goes to the database.
 */
export const SERIAL_SEND_ECHO_STORAGE_KEY = "tterm.serial.sendEcho"
/** Connections remembered; the least recently changed is forgotten first. */
const CONNECTION_LIMIT = 50

function readHidden(): string[] {
  try {
    const stored = JSON.parse(localStorage.getItem(SERIAL_SEND_ECHO_STORAGE_KEY) ?? "[]") as unknown
    return Array.isArray(stored)
      ? stored.filter((key): key is string => typeof key === "string")
      : []
  } catch {
    return []
  }
}

/** The key a connection's history is kept under: its saved profile, else its port. */
export function serialSendKey(profileId: string | undefined, port: string): string {
  return profileId ? `profile:${profileId}` : `port:${port}`
}

/** Whether the connection's send bar shows what it sends in the terminal. */
export function loadSerialSendEcho(key: string): boolean {
  return !readHidden().includes(key)
}

export function saveSerialSendEcho(key: string, echo: boolean) {
  const others = readHidden().filter((item) => item !== key)
  const hidden = echo ? others : [...others, key].slice(-CONNECTION_LIMIT)
  try {
    localStorage.setItem(SERIAL_SEND_ECHO_STORAGE_KEY, JSON.stringify(hidden))
  } catch {
    // A full or blocked storage only costs the choice.
  }
}
