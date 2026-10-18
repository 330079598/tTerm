/** How the send bar sends: text the way typing does, or hex bytes as they are. */
export type SerialSendMode = "text" | "hex"

/** One thing the send bar sent, for its history. */
export interface SerialSendEntry {
  mode: SerialSendMode
  text: string
}

/** What a serial connection's send bar remembers between sessions. */
export interface SerialSendMemory {
  history: SerialSendEntry[]
  /** Show what is sent in the terminal. */
  echo: boolean
}

export const SERIAL_SEND_STORAGE_KEY = "tterm.serial.send"
export const SERIAL_SEND_HISTORY_LIMIT = 30
/** Connections remembered; the least recently used is forgotten first. */
const CONNECTION_LIMIT = 50

export const DEFAULT_SERIAL_SEND_MEMORY: SerialSendMemory = { history: [], echo: true }

function readAll(): Record<string, SerialSendMemory> {
  try {
    const stored = JSON.parse(localStorage.getItem(SERIAL_SEND_STORAGE_KEY) ?? "{}") as unknown
    return stored && typeof stored === "object" ? (stored as Record<string, SerialSendMemory>) : {}
  } catch {
    return {}
  }
}

function isEntry(value: unknown): value is SerialSendEntry {
  if (!value || typeof value !== "object") return false
  const { mode, text } = value as Record<string, unknown>
  return (mode === "text" || mode === "hex") && typeof text === "string"
}

/** The key a connection's memory is kept under: its saved profile, else its port. */
export function serialSendKey(profileId: string | undefined, port: string): string {
  return profileId ? `profile:${profileId}` : `port:${port}`
}

export function loadSerialSendMemory(key: string): SerialSendMemory {
  const stored = readAll()[key]
  if (!stored || typeof stored !== "object") return DEFAULT_SERIAL_SEND_MEMORY
  return {
    history: Array.isArray(stored.history)
      ? stored.history.filter(isEntry).slice(0, SERIAL_SEND_HISTORY_LIMIT)
      : [],
    echo: stored.echo !== false,
  }
}

export function saveSerialSendMemory(key: string, memory: SerialSendMemory) {
  const all = readAll()
  delete all[key]
  // Insertion order is use order: the newest goes last, the oldest is dropped.
  const kept = Object.entries(all).slice(-(CONNECTION_LIMIT - 1))
  try {
    localStorage.setItem(
      SERIAL_SEND_STORAGE_KEY,
      JSON.stringify(Object.fromEntries([...kept, [key, memory]]))
    )
  } catch {
    // A full or blocked storage only costs the history.
  }
}

/** `entry` first in `history`, once. */
export function withSentEntry(
  history: SerialSendEntry[],
  entry: SerialSendEntry
): SerialSendEntry[] {
  return [
    entry,
    ...history.filter((item) => item.mode !== entry.mode || item.text !== entry.text),
  ].slice(0, SERIAL_SEND_HISTORY_LIMIT)
}
