// Replaces `@tauri-apps/api/core` when Vite runs with `--mode parity`.
//
// The reference capture needs the real React components, not a second copy of
// them, so the modules below answer the three things the frontend uses from
// Tauri: `invoke`, events and `Channel`. A command without a fixture throws,
// which makes a screen that reaches for an unrecorded command fail loudly
// instead of screenshotting an empty state. The imported types are the real
// package's types, so the mock cannot silently drift from the IPC contract.

import type { InvokeArgs, InvokeOptions } from "@tauri-apps/api/core"
import type { ParityFixture } from "@/parity/fixtures"

/** Matches Tauri's key for custom IPC serialisation. */
export const SERIALIZE_TO_IPC_FN = "__TAURI_TO_IPC_KEY__"

let activeFixture: ParityFixture | null = null
const invoked: string[] = []
const channels = new Map<number, Channel<unknown>>()
let nextChannelId = 1

/** Makes `fixture` the answers the next `invoke` calls return. */
export function activateFixture(fixture: ParityFixture): void {
  activeFixture = fixture
  invoked.length = 0
}

/** The distinct commands the screen called, sorted, for the layout JSON. */
export function invokedCommands(): string[] {
  return [...new Set(invoked)].sort()
}

/** Restores the mock to its initial state; tests use this between cases. */
export function resetParityCore(): void {
  activeFixture = null
  invoked.length = 0
  channels.clear()
  nextChannelId = 1
}

/**
 * A Tauri channel, addressed by number on the Rust side. Fixtures cannot push
 * messages yet, so `sendChannelMessage` is the harness's stand-in for the
 * backend sending one.
 */
export class Channel<T = unknown> {
  readonly id: number
  #onmessage: (response: T) => void = () => {}

  constructor(onmessage?: (response: T) => void) {
    this.id = nextChannelId++
    channels.set(this.id, this as unknown as Channel<unknown>)
    if (onmessage) this.#onmessage = onmessage
  }

  set onmessage(handler: (response: T) => void) {
    this.#onmessage = handler
  }

  get onmessage(): (response: T) => void {
    return this.#onmessage
  }

  [SERIALIZE_TO_IPC_FN](): string {
    return String(this.id)
  }

  toJSON(): string {
    return String(this.id)
  }
}

/** Delivers a backend message to the channel with `id`. */
export function sendChannelMessage(id: number, payload: unknown): void {
  const channel = channels.get(id)
  if (!channel) throw new Error(`Parity mock: no channel with id ${id}`)
  channel.onmessage(payload)
}

/** Returns the fixture value of `command`, cloned so screens cannot mutate it. */
export async function invoke<T>(
  command: string,
  args?: InvokeArgs,
  options?: InvokeOptions
): Promise<T> {
  // The parameters exist to keep the shape of Tauri's signature; fixtures are
  // addressed by command name alone.
  void args
  void options

  if (!activeFixture) {
    throw new Error("Parity mock: no fixture is active; the screen is not in parity/screens.json")
  }
  invoked.push(command)
  if (!Object.prototype.hasOwnProperty.call(activeFixture.commands, command)) {
    const known = Object.keys(activeFixture.commands).sort().join(", ")
    throw new Error(`Parity mock: no fixture for "${command}"; fixtures provide: ${known}`)
  }
  const value = activeFixture.commands[command]
  return (value === undefined ? undefined : structuredClone(value)) as T
}

/** True when running inside Tauri, which parity mode emulates. */
export function isTauri(): boolean {
  return true
}

/** Mirrors Tauri's asset URL helper; parity screens do not load assets yet. */
export function convertFileSrc(filePath: string, protocol = "asset"): string {
  return `${protocol}://localhost/${encodeURIComponent(filePath)}`
}
