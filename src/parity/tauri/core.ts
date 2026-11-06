// Replaces `@tauri-apps/api/core` when Vite runs with `--mode parity`.
//
// The reference capture needs the real React components, not a second copy of
// them, so the modules below answer the three things the frontend uses from
// Tauri: `invoke`, events and `Channel`. A command without a fixture throws,
// which makes a screen that reaches for an unrecorded command fail loudly
// instead of screenshotting an empty state. Every such command is also recorded
// in `window.__PARITY_MISSING__`, because a component is free to catch the
// rejection and carry on; the capture checks that list after the screen is
// ready. The imported types are the real package's types, so the mock cannot
// silently drift from the IPC contract.

import type { InvokeArgs, InvokeOptions } from "@tauri-apps/api/core"
import type { ParityFixture } from "@/parity/fixtures"

/** Matches Tauri's key for custom IPC serialisation. */
export const SERIALIZE_TO_IPC_FN = "__TAURI_TO_IPC_KEY__"

type JsonObject = Record<string, unknown>

interface RejectDirective {
  $reject: string
}

interface PendingDirective {
  $pending: true
}

interface ArgsCase {
  args: unknown
  value: unknown
}

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
  if (typeof window !== "undefined") window.__PARITY_MISSING__ = []
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

/**
 * Returns the fixture value of `command`, cloned so screens cannot mutate it.
 *
 * A value may be a plain value, `{ "$reject": "text" }` to reject,
 * `{ "$pending": true }` to never settle, or a non-empty list of
 * `{ "args": {...}, "value": ... }` cases matched exactly against `args`.
 */
export async function invoke<T>(
  command: string,
  args?: InvokeArgs,
  options?: InvokeOptions
): Promise<T> {
  void options
  invoked.push(command)

  if (!activeFixture) {
    recordMissing(command)
    throw new Error("Parity mock: no fixture is active; the screen is not in parity/screens.json")
  }
  if (!Object.prototype.hasOwnProperty.call(activeFixture.commands, command)) {
    recordMissing(command)
    const known = Object.keys(activeFixture.commands).sort().join(", ")
    throw new Error(`Parity mock: no fixture for "${command}"; fixtures provide: ${known}`)
  }

  const fixture = activeFixture.commands[command]
  const value = valueForArgs(command, fixture, args)
  if (isReject(value)) throw new Error(value.$reject)
  if (isPending(value)) return new Promise<T>(() => {})
  return (value === undefined ? undefined : structuredClone(value)) as T
}

function valueForArgs(command: string, fixture: unknown, args: InvokeArgs | undefined): unknown {
  if (!isArgsList(fixture)) return fixture
  const match = fixture.find((candidate) => deepEqual(candidate.args, args))
  if (!match) {
    recordMissing(command)
    throw new Error(
      `Parity mock: no args fixture for "${command}"; ${fixture.length} case(s) did not match`
    )
  }
  return match.value
}

function isReject(value: unknown): value is RejectDirective {
  return isObject(value) && typeof value.$reject === "string"
}

function isPending(value: unknown): value is PendingDirective {
  return isObject(value) && value.$pending === true
}

function isArgsCase(value: unknown): value is ArgsCase {
  return isObject(value) && "args" in value && "value" in value
}

/** A list of argument cases. An empty list is a normal empty array fixture. */
function isArgsList(value: unknown): value is ArgsCase[] {
  return Array.isArray(value) && value.length > 0 && value.every(isArgsCase)
}

function isObject(value: unknown): value is JsonObject {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function deepEqual(first: unknown, second: unknown): boolean {
  if (first === second) return true
  if (typeof first !== typeof second) return false
  if (Array.isArray(first) || Array.isArray(second)) {
    if (!Array.isArray(first) || !Array.isArray(second) || first.length !== second.length) {
      return false
    }
    return first.every((value, index) => deepEqual(value, second[index]))
  }
  if (isObject(first) && isObject(second)) {
    const keys = Object.keys(first).sort()
    const otherKeys = Object.keys(second).sort()
    if (!deepEqual(keys, otherKeys)) return false
    return keys.every((key) => deepEqual(first[key], second[key]))
  }
  return false
}

function recordMissing(command: string): void {
  if (typeof window === "undefined") return
  const missing = window.__PARITY_MISSING__ ?? []
  if (!missing.includes(command)) missing.push(command)
  window.__PARITY_MISSING__ = missing
}

/** True when running inside Tauri, which parity mode emulates. */
export function isTauri(): boolean {
  return true
}

/** Mirrors Tauri's asset URL helper; parity screens do not load assets yet. */
export function convertFileSrc(filePath: string, protocol = "asset"): string {
  return `${protocol}://localhost/${encodeURIComponent(filePath)}`
}
