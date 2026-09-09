import { getKeymapAction, type KeymapActionId } from "@/lib/keymap/actions"
import { eventToChord, isEditableTarget, serializeChord, type Chord } from "@/lib/keymap/chord"

/**
 * Handler for a dispatched action. Return `false` to decline handling —
 * e.g. when an action's own preconditions (selection, loading state) are
 * not met — so the key keeps flowing to the terminal. Any other return
 * value (or none) consumes the event.
 */
export type KeymapHandler = (event: KeyboardEvent, chord: Chord) => void | boolean

export interface KeymapDispatchEnvironment {
  /** Whether dispatching is paused (settings key recorder, ...). */
  isDispatchSuppressed(): boolean
  /** Whether a dialog, menu, popover, or similar blocking layer is open. */
  hasOpenLayer(): boolean
  /** Serialized chord -> actionId lookup for the current bindings. */
  getChordIndex(): Map<string, KeymapActionId>
  /** Registered handlers per action. */
  getHandlers(): Map<KeymapActionId, Set<KeymapHandler>>
  /** Platform modifier resolution (Cmd vs Ctrl). */
  isMac(): boolean
}

/**
 * Resolve one keydown event against the bindings. Returns the matched action
 * id when the event was consumed. Pure — unit-testable without a DOM.
 */
export function dispatchKeydownEvent(
  event: KeyboardEvent,
  env: KeymapDispatchEnvironment
): KeymapActionId | null {
  if (env.isDispatchSuppressed() || env.hasOpenLayer()) {
    return null
  }
  if (event.key === "Escape") {
    // Escape stays owned by whichever component is open (dialogs, search
    // bars, menus); it is deliberately not part of the registry.
    return null
  }

  const chord = eventToChord(event, env.isMac())
  if (!chord) {
    return null
  }
  const serialized = serializeChord(chord)
  if (!serialized) {
    return null
  }

  const index = env.getChordIndex()
  let actionId = index.get(serialized)
  if (!actionId && env.isMac() && chord.ctrl && !chord.mod) {
    // Keep the app's historical macOS behavior where Ctrl also activates a
    // Mod binding, while giving explicit `ctrl+...` bindings precedence.
    actionId = index.get(serializeChord({ ...chord, ctrl: undefined, mod: true }) ?? "")
  } else if (!actionId && !env.isMac() && chord.mod) {
    // Ctrl is Mod on Windows/Linux, so explicit Ctrl bindings work there too.
    actionId = index.get(serializeChord({ ...chord, mod: undefined, ctrl: true }) ?? "")
  }
  if (!actionId) {
    // No binding for this chord: do not intercept — the key flows to the
    // terminal (Vim, tmux, readline passthrough is the default behavior).
    return null
  }

  const definition = getKeymapAction(actionId)
  if (definition && !definition.allowInEditable && isEditableTarget(event.target)) {
    return null
  }

  const handlers = env.getHandlers().get(actionId)
  if (!handlers || handlers.size === 0) {
    // Bound but nothing mounted to serve it right now (e.g. no active
    // terminal): don't swallow the key.
    return null
  }

  const results: (void | boolean)[] = []
  for (const handler of handlers) {
    results.push(handler(event, chord))
  }
  if (!results.some((result) => result !== false)) {
    // Every registered handler declined: pass the key through.
    return null
  }

  event.preventDefault()
  event.stopPropagation()
  return actionId
}

/**
 * Installs the single global capture-phase keydown listener. Returns a
 * detach function.
 */
export function attachKeymapDispatcher(target: Window, env: KeymapDispatchEnvironment): () => void {
  const listener = (event: KeyboardEvent) => {
    dispatchKeydownEvent(event, env)
  }
  target.addEventListener("keydown", listener, true)
  return () => {
    target.removeEventListener("keydown", listener, true)
  }
}
