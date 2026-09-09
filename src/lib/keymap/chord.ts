/**
 * Chord model for keymap bindings.
 *
 * A chord is a normalized representation of a keyboard shortcut, e.g.
 * `mod+shift+p`. `mod` is the platform primary modifier: Cmd on macOS,
 * Ctrl elsewhere. Explicit `ctrl` bindings stay physical Ctrl on macOS;
 * the dispatcher falls back from Ctrl to Mod there to preserve the app's
 * historical dual-matching behavior when no explicit Ctrl binding exists.
 */

import { platform } from "@tauri-apps/plugin-os"

export interface Chord {
  /** Normalized key: single character or a named key like `tab` / `pagedown`. */
  key: string
  mod?: boolean
  /** Physical Ctrl, used for bindings that must not become Cmd on macOS. */
  ctrl?: boolean
  alt?: boolean
  shift?: boolean
}

const MODIFIER_KEYS = new Set(["Shift", "Control", "Alt", "Meta"])

/** Keys that can never be bound (pure modifier presses, IME intermediates). */
const UNBINDABLE_KEYS = new Set(["Dead", "Unidentified", "Process", "Compose"])

/** Chord serialization order. */
const MODIFIER_ORDER = ["mod", "ctrl", "alt", "shift"] as const

/** Modifier tokens — never valid as the key part of a chord. */
const MODIFIER_TOKENS = new Set<string>(MODIFIER_ORDER)

/** Keys that need an alias to keep the `+`-separated serialization unambiguous. */
const KEY_ALIASES: Record<string, string> = {
  " ": "space",
  "+": "plus",
}

/** Physical printable keys mapped to the existing serialized key tokens. */
const CODE_KEY_ALIASES: Record<string, string> = {
  Space: "space",
  Backquote: "`",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
}

const KEY_ALIAS_DISPLAY: Record<string, string> = {
  space: "Space",
  plus: "+",
  escape: "Esc",
  tab: "Tab",
  enter: "Enter",
  backspace: "Backspace",
  delete: "Del",
  home: "Home",
  end: "End",
  pagedown: "PageDown",
  pageup: "PageUp",
  insert: "Ins",
  arrowup: "↑",
  arrowdown: "↓",
  arrowleft: "←",
  arrowright: "→",
}

const NAMED_KEY_RE = /^[a-z][a-z0-9]*$/
const SINGLE_CHAR_KEY_RE = /^[a-z0-9\\/\-[\]`;='.,]$/

let _cachedIsMac: boolean | null = null

/**
 * Whether the primary modifier is Cmd (macOS). Follows the same detection
 * convention as useTerminalLifecycle: the Tauri plugin first, UA fallback.
 */
export function isMacPlatform(): boolean {
  if (_cachedIsMac !== null) {
    return _cachedIsMac
  }
  try {
    _cachedIsMac = platform() === "macos"
  } catch {
    if (typeof navigator !== "undefined") {
      const platformHint = `${navigator.platform} ${navigator.userAgent}`.toLowerCase()
      _cachedIsMac = platformHint.includes("mac")
    } else {
      _cachedIsMac = false
    }
  }
  return _cachedIsMac
}

/**
 * Normalizes `event.key` into a bindable key token, or null when the key
 * cannot participate in a chord (pure modifiers, IME dead keys, ...).
 */
export function normalizeEventKey(rawKey: string): string | null {
  if (!rawKey || MODIFIER_KEYS.has(rawKey) || UNBINDABLE_KEYS.has(rawKey)) {
    return null
  }
  const aliased = KEY_ALIASES[rawKey]
  if (aliased) {
    return aliased
  }
  if (rawKey.length === 1) {
    const lower = rawKey.toLowerCase()
    return SINGLE_CHAR_KEY_RE.test(lower) ? lower : null
  }
  const lower = rawKey.toLowerCase()
  return NAMED_KEY_RE.test(lower) ? lower : null
}

/**
 * Maps a KeyboardEvent `code` to the stable token used by persisted chords.
 * Letter and digit codes follow their physical US-keyboard positions, so a
 * binding keeps working when the active keyboard layout changes.
 */
export function normalizeEventCode(rawCode: string): string | null {
  if (!rawCode) {
    return null
  }
  if (/^Key[A-Z]$/.test(rawCode)) {
    return rawCode.slice(3).toLowerCase()
  }
  if (/^Digit[0-9]$/.test(rawCode)) {
    return rawCode.slice(5)
  }
  const aliased = CODE_KEY_ALIASES[rawCode]
  if (aliased) {
    return aliased
  }
  return normalizeEventKey(rawCode)
}

export function eventToChord(event: KeyboardEvent, isMac: boolean): Chord | null {
  if (event.isComposing || UNBINDABLE_KEYS.has(event.key) || MODIFIER_KEYS.has(event.key)) {
    return null
  }
  const key = normalizeEventCode(event.code) ?? normalizeEventKey(event.key)
  if (!key) {
    return null
  }
  return {
    key,
    mod: isMac ? event.metaKey || undefined : event.ctrlKey || undefined,
    ctrl: isMac ? event.ctrlKey || undefined : undefined,
    alt: event.altKey || undefined,
    shift: event.shiftKey || undefined,
  }
}

export function serializeChord(chord: Chord): string | null {
  const parts: string[] = []
  for (const modifier of MODIFIER_ORDER) {
    if (chord[modifier]) {
      parts.push(modifier)
    }
  }
  if (!chord.key) {
    return null
  }
  parts.push(chord.key)
  return parts.join("+")
}

export function parseChord(serialized: string): Chord | null {
  const parts = serialized
    .split("+")
    .map((part) => part.trim().toLowerCase())
    .filter(Boolean)
  if (parts.length === 0) {
    return null
  }

  const chord: Chord = { key: "" }
  for (let index = 0; index < parts.length - 1; index += 1) {
    const part = parts[index]
    if (part === "mod") {
      if (chord.mod) return null
      chord.mod = true
    } else if (part === "ctrl") {
      if (chord.ctrl) return null
      chord.ctrl = true
    } else if (part === "alt") {
      if (chord.alt) return null
      chord.alt = true
    } else if (part === "shift") {
      if (chord.shift) return null
      chord.shift = true
    } else {
      return null
    }
  }

  const key = parts[parts.length - 1]
  if (!key || MODIFIER_TOKENS.has(key)) {
    return null
  }
  if (!(SINGLE_CHAR_KEY_RE.test(key) || NAMED_KEY_RE.test(key))) {
    return null
  }
  chord.key = key
  return chord
}

export function chordsEqual(a: Chord, b: Chord): boolean {
  return (
    a.key === b.key &&
    Boolean(a.mod) === Boolean(b.mod) &&
    Boolean(a.ctrl) === Boolean(b.ctrl) &&
    Boolean(a.alt) === Boolean(b.alt) &&
    Boolean(a.shift) === Boolean(b.shift)
  )
}

function formatKeyForDisplay(key: string): string {
  return KEY_ALIAS_DISPLAY[key] ?? (key.length === 1 ? key.toUpperCase() : key)
}

/** Human-readable display, platform aware (macOS uses ⌘/⌥/⇧ symbols). */
export function formatChord(serialized: string, isMac: boolean): string {
  const chord = parseChord(serialized)
  if (!chord) {
    return serialized
  }
  const key = formatKeyForDisplay(chord.key)
  if (isMac) {
    return `${chord.mod ? "⌘" : ""}${chord.ctrl ? "⌃" : ""}${chord.alt ? "⌥" : ""}${chord.shift ? "⇧" : ""}${key}`
  }
  return `${chord.mod || chord.ctrl ? "Ctrl+" : ""}${chord.alt ? "Alt+" : ""}${chord.shift ? "Shift+" : ""}${key}`
}

/**
 * Whether the event target is an editable surface (inputs, textareas,
 * contenteditable, selects). The xterm helper textarea also matches, which is
 * what keeps terminal readline chords (Ctrl+A, Ctrl+V, ...) flowing through
 * to the remote shell for actions marked `allowInEditable: false`.
 */
export function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false
  }

  return Boolean(
    target.isContentEditable ||
    target.closest("input, textarea, select, [contenteditable='true'], [role='textbox']")
  )
}
