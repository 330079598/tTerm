import { KEYMAP_ACTIONS, KEYMAP_ACTION_IDS, type KeymapActionId } from "@/lib/keymap/actions"
import { parseChord, serializeChord } from "@/lib/keymap/chord"

/**
 * Persisted keymap configuration. `bindings` is a sparse override layer:
 * a missing entry means "use the tTerm default", an explicit `null`
 * means "unbound", and an array of serialized chords replaces the default.
 */
export interface KeymapConfig {
  bindings: Partial<Record<KeymapActionId, string[] | null>>
}

export const DEFAULT_KEYMAP_CONFIG: KeymapConfig = {
  bindings: {},
}

/** Serialized default chord (or chords) per action. `null` means unbound. */
const DEFAULT_BINDINGS: Record<KeymapActionId, string | string[] | null> = {
  "workspace.newTab": "ctrl+shift+t",
  "workspace.splitRight": "mod+\\",
  "workspace.splitBelow": "mod+shift+\\",
  "commandLibrary.open": "mod+shift+p",
  "broadcast.togglePanel": "mod+shift+b",
  "tabs.next": "ctrl+tab",
  "tabs.prev": "ctrl+shift+tab",
  "tabs.switchToNth": [
    "alt+1",
    "alt+2",
    "alt+3",
    "alt+4",
    "alt+5",
    "alt+6",
    "alt+7",
    "alt+8",
    "alt+9",
  ],
  "tabs.closeActive": "mod+shift+w",
  "terminal.find": "mod+f",
  "terminal.clear": "mod+shift+k",
  "terminal.saveSelection": "mod+shift+s",
  "sftp.toggle": "ctrl+t",
  "sftp.selectAll": "mod+a",
  "sftp.focusPath": "mod+l",
  "sftp.pasteUpload": "mod+v",
  "zmodem.sendFiles": null,
  "zmodem.receiveFiles": null,
  "editor.save": "mod+s",
}

/** actionId -> serialized chords currently in effect (null = unbound). */
export type EffectiveKeymap = Record<KeymapActionId, string[] | null>

function normalizeBinding(source: string | string[] | null | undefined): string[] | null {
  if (source == null) {
    return null
  }
  const valid = (Array.isArray(source) ? source : [source])
    .map(parseChord)
    .filter((chord): chord is NonNullable<typeof chord> => chord !== null)
    .map((chord) => serializeChord(chord))
    .filter((serialized): serialized is string => serialized !== null)
  return valid.length > 0 ? valid : null
}

export function resolveEffectiveKeymap(keymap: KeymapConfig): EffectiveKeymap {
  const effective = {} as EffectiveKeymap

  for (const action of KEYMAP_ACTIONS) {
    const override = keymap.bindings[action.id]
    if (override === null) {
      effective[action.id] = null
      continue
    }
    const normalizedOverride = normalizeBinding(override)
    effective[action.id] = normalizedOverride ?? normalizeBinding(DEFAULT_BINDINGS[action.id])
  }

  return effective
}

/**
 * Chords bound to more than one action. These must be resolved in the
 * settings UI; the runtime index resolves them deterministically to the
 * first action, but they are never expected to persist.
 */
export function findKeymapConflicts(effective: EffectiveKeymap): Map<string, KeymapActionId[]> {
  const chordOwners = new Map<string, KeymapActionId[]>()
  for (const action of KEYMAP_ACTIONS) {
    for (const chord of effective[action.id] ?? []) {
      const owners = chordOwners.get(chord)
      if (owners) {
        if (!owners.includes(action.id)) {
          owners.push(action.id)
        }
      } else {
        chordOwners.set(chord, [action.id])
      }
    }
  }

  const conflicts = new Map<string, KeymapActionId[]>()
  for (const [chord, owners] of chordOwners) {
    if (owners.length > 1) {
      conflicts.set(chord, owners)
    }
  }
  return conflicts
}

/** serialized chord -> actionId, for O(1) dispatch lookups. */
export function buildChordIndex(effective: EffectiveKeymap): Map<string, KeymapActionId> {
  const index = new Map<string, KeymapActionId>()
  for (const action of KEYMAP_ACTIONS) {
    for (const chord of effective[action.id] ?? []) {
      if (!index.has(chord)) {
        index.set(chord, action.id)
      }
    }
  }
  return index
}

/**
 * Chords known to collide with common remote-side tooling (readline, Emacs,
 * tmux, shell flow control). Purely advisory — surfaced as a warning in the
 * keymap settings and never enforced.
 */
export const KNOWN_CLI_CONFLICT_HINTS: Record<string, string> = {
  "mod+s": "xoff",
  "mod+q": "xon",
  "mod+a": "readlineHome",
  "mod+e": "readlineEnd",
  "mod+u": "readlineKillLineStart",
  "mod+k": "readlineKillLineEnd",
  "mod+w": "readlineDeleteWord",
  "mod+l": "clearScreen",
  "mod+r": "historySearch",
  "mod+b": "tmuxPrefix",
  "mod+z": "suspend",
  "mod+d": "eof",
  "mod+v": "quotedInsert",
  "mod+n": "historyNext",
  "mod+p": "historyPrevious",
  "mod+o": "operateAndGetNext",
  "alt+b": "wordBack",
  "alt+f": "wordForward",
  "alt+d": "wordKill",
}

export function getCliConflictHint(chord: string, isMac = false): string | undefined {
  if (isMac && chord.startsWith("mod+")) {
    return undefined
  }
  const physicalCtrlChord = chord.startsWith("ctrl+") ? `mod+${chord.slice(5)}` : chord
  return KNOWN_CLI_CONFLICT_HINTS[physicalCtrlChord]
}

export function normalizeKeymap(value: unknown): KeymapConfig {
  if (typeof value !== "object" || value === null) {
    return DEFAULT_KEYMAP_CONFIG
  }

  const raw = value as Partial<KeymapConfig>
  const bindings: Partial<Record<KeymapActionId, string[] | null>> = {}

  if (typeof raw.bindings === "object" && raw.bindings !== null) {
    for (const [actionId, rawChords] of Object.entries(raw.bindings)) {
      if (!KEYMAP_ACTION_IDS.includes(actionId as KeymapActionId)) {
        continue
      }
      if (rawChords === null) {
        bindings[actionId as KeymapActionId] = null
        continue
      }
      const chords = (Array.isArray(rawChords) ? rawChords : [rawChords]).filter(
        (chord): chord is string => typeof chord === "string"
      )
      const valid = chords
        .map((chord) => parseChord(chord))
        .filter((chord): chord is NonNullable<typeof chord> => chord !== null)
        .map((chord) => serializeChord(chord))
        .filter((serialized): serialized is string => serialized !== null)
      if (valid.length > 0) {
        bindings[actionId as KeymapActionId] = valid
      }
    }
  }

  return { bindings }
}
