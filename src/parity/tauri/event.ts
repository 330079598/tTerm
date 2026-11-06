// Replaces `@tauri-apps/api/event` when Vite runs with `--mode parity`.
// Listeners are kept in memory; `emit` delivers to them so a test or a future
// fixture-driven harness can simulate a backend event. Event ids increment the
// same way Tauri's do, which keeps the payload shape identical.

import type { Event, EventCallback, EventName, Options, UnlistenFn } from "@tauri-apps/api/event"

type AnyEvent = Event<unknown>
type AnyHandler = (event: AnyEvent) => void

const listeners = new Map<string, Set<AnyHandler>>()
let nextEventId = 1

/** Registers `handler` for `event` and returns the function that removes it. */
export async function listen<T>(
  event: EventName,
  handler: EventCallback<T>,
  options?: Options
): Promise<UnlistenFn> {
  // Plugin targets do not exist in the reference capture.
  void options
  const key = String(event)
  const handlers = listeners.get(key) ?? new Set<AnyHandler>()
  const wrapped = handler as AnyHandler
  handlers.add(wrapped)
  listeners.set(key, handlers)
  return () => {
    handlers.delete(wrapped)
    if (handlers.size === 0) listeners.delete(key)
  }
}

/** Registers a one-shot listener. */
export async function once<T>(
  event: EventName,
  handler: EventCallback<T>,
  options?: Options
): Promise<UnlistenFn> {
  const unlisten = await listen<T>(
    event,
    (received) => {
      unlisten()
      handler(received)
    },
    options
  )
  return unlisten
}

/** Delivers `payload` to the listeners of `event`. */
export async function emit(event: string, payload?: unknown): Promise<void> {
  deliver(event, payload)
}

/** Delivers `payload` to the listeners of `event`; targets are ignored. */
export async function emitTo(
  target: string | { kind: string; label: string },
  event: string,
  payload?: unknown
): Promise<void> {
  void target
  deliver(event, payload)
}

/** The event names Tauri defines, for compatibility with frontend imports. */
export const TauriEvent = {
  WINDOW_RESIZED: "tauri://resize",
  WINDOW_MOVED: "tauri://move",
  WINDOW_CLOSE_REQUESTED: "tauri://close-requested",
  WINDOW_DESTROYED: "tauri://destroyed",
  WINDOW_FOCUS: "tauri://focus",
  WINDOW_BLUR: "tauri://blur",
  WINDOW_SCALE_FACTOR_CHANGED: "tauri://scale-change",
  WINDOW_THEME_CHANGED: "tauri://theme-changed",
  DRAG_ENTER: "tauri://drag-enter",
  DRAG_OVER: "tauri://drag-over",
  DRAG_DROP: "tauri://drag-drop",
  DRAG_LEAVE: "tauri://drag-leave",
} as const

/** Removes every listener; tests use this between cases. */
export function resetParityEvents(): void {
  listeners.clear()
  nextEventId = 1
}

function deliver(event: string, payload: unknown): void {
  const handlers = listeners.get(event)
  if (!handlers) return
  const received: AnyEvent = {
    event,
    id: nextEventId++,
    payload,
  }
  for (const handler of [...handlers]) handler(received)
}
