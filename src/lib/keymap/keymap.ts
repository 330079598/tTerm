import { KEYMAP_ACTIONS, KEYMAP_ACTION_IDS, type KeymapActionId } from "@/lib/keymap/actions"
import { formatChord, parseChord, serializeChord } from "@/lib/keymap/chord"

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

/** The nine chords that switch to tabs 1-9, all on the given modifier. */
function digitChords(modifier: string): string[] {
  return Array.from({ length: 9 }, (_, index) => `${modifier}+${index + 1}`)
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
  "tabs.switchToNth": digitChords("alt"),
  "tabs.renameActive": "mod+shift+r",
  "tabs.closeActive": "mod+shift+w",
  "terminal.find": "mod+f",
  "terminal.clear": "mod+shift+k",
  "terminal.reset": null,
  "terminal.saveSelection": "mod+shift+s",
  "terminal.fillSavedPassword": "mod+shift+enter",
  // Shift+= is "+" on US layouts, so Ctrl++ zooms in as well.
  "terminal.zoomIn": ["mod+=", "mod+shift+=", "mod+numpadadd"],
  "terminal.zoomOut": ["mod+-", "mod+numpadsubtract"],
  "terminal.zoomReset": ["mod+0", "mod+numpad0"],
  // Only taken from the shell once it has marked a prompt (OSC 133).
  "terminal.previousCommand": "mod+arrowup",
  "terminal.nextCommand": "mod+arrowdown",
  "terminal.copyLastCommandOutput": null,
  "sftp.toggle": "ctrl+t",
  "sftp.selectAll": "mod+a",
  "sftp.focusPath": "mod+l",
  "sftp.pasteUpload": "mod+v",
  // Alt+Arrow follows Windows Explorer and Linux file managers; Mod+[ / Mod+]
  // is the macOS Finder chord, which mouse utilities there often map side
  // buttons to.
  "sftp.back": ["alt+arrowleft", "mod+["],
  "sftp.forward": ["alt+arrowright", "mod+]"],
  "sftp.up": "alt+arrowup",
  "zmodem.sendFiles": null,
  "zmodem.receiveFiles": null,
  "editor.save": "mod+s",
}

/**
 * Defaults that differ on macOS. Cmd+1-9 switches tabs in Terminal, iTerm2
 * and browsers there, while Option+digit types symbols such as "¡" and is
 * Meta+digit (a readline count) once Option acts as Meta.
 */
const MAC_DEFAULT_BINDINGS: Partial<Record<KeymapActionId, string | string[] | null>> = {
  "tabs.switchToNth": digitChords("mod"),
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

export function resolveEffectiveKeymap(keymap: KeymapConfig, isMac = false): EffectiveKeymap {
  const effective = {} as EffectiveKeymap
  const defaults = isMac ? { ...DEFAULT_BINDINGS, ...MAC_DEFAULT_BINDINGS } : DEFAULT_BINDINGS

  for (const action of KEYMAP_ACTIONS) {
    const override = keymap.bindings[action.id]
    if (override === null) {
      effective[action.id] = null
      continue
    }
    const normalizedOverride = normalizeBinding(override)
    effective[action.id] = normalizedOverride ?? normalizeBinding(defaults[action.id])
  }

  return effective
}

/** How an action's first chord reads in menus and tooltips, or undefined while it is unbound. */
export function getShortcutLabel(
  effective: EffectiveKeymap,
  actionId: KeymapActionId,
  isMac: boolean
): string | undefined {
  const chord = effective[actionId]?.[0]
  return chord ? formatChord(chord, isMac) : undefined
}

/** A button's tooltip: its label, followed by its shortcut when it has one. */
export function withShortcut(label: string, shortcut: string | undefined): string {
  return shortcut ? `${label} (${shortcut})` : label
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
