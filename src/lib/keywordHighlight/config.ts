// Keyword highlighting colors words such as "error" or an IP address wherever
// they appear in a terminal's output, like Xshell's highlight sets. Rules are
// grouped in named sets; one set is the default, and a connection may pick
// another or none.

/** The terminal palette entries a rule's color may name. */
export const HIGHLIGHT_PALETTE_COLORS = [
  "red",
  "green",
  "yellow",
  "blue",
  "magenta",
  "cyan",
  "white",
  "black",
  "brightRed",
  "brightGreen",
  "brightYellow",
  "brightBlue",
  "brightMagenta",
  "brightCyan",
  "brightWhite",
  "brightBlack",
] as const

export type HighlightPaletteColor = (typeof HIGHLIGHT_PALETTE_COLORS)[number]

export interface HighlightRule {
  id: string
  enabled: boolean
  pattern: string
  /** A keyword matches its text literally; a regex is a JavaScript pattern. */
  kind: "keyword" | "regex"
  case_sensitive: boolean
  /** Only where no letter, digit or underscore touches either end. */
  whole_word: boolean
  /** A palette color name or `#rrggbb`; null keeps the text's color. */
  foreground: string | null
  /** A palette color name or `#rrggbb`; null keeps the background. */
  background: string | null
}

export interface HighlightSet {
  id: string
  /** Empty for the built-in set, which shows a translated name. */
  name: string
  rules: HighlightRule[]
}

export interface KeywordHighlightConfig {
  enabled: boolean
  /** The set terminals use unless their connection picks another. */
  default_set: string
  /** Also highlight full-screen programs such as vim, htop or tmux. */
  alternate_screen: boolean
  /** Null until the user edits a set: the built-in sets apply. */
  sets: HighlightSet[] | null
}

/** A connection's choice: follow the default set, no highlighting, or a set's id. */
export const HIGHLIGHT_SET_FOLLOW_DEFAULT = "default"
export const HIGHLIGHT_SET_OFF = "off"

export const BUILTIN_HIGHLIGHT_SET_ID = "builtin"

const OCTET = "(?:25[0-5]|2[0-4]\\d|1\\d\\d|[1-9]?\\d)"
const HEX4 = "[\\da-f]{1,4}"

/** Dotted quads with an optional prefix length, not inside longer dotted numbers. */
const IPV4_PATTERN = `(?<![\\w.])${OCTET}(?:\\.${OCTET}){3}(?:/(?:3[0-2]|[12]?\\d))?(?![\\w]|\\.\\d)`

/**
 * Full and `::`-compressed addresses. The lookahead wants a digit among the
 * address's characters, so words made of hex letters, such as `Add::add` in
 * Rust or C++ output, are left alone.
 */
const IPV6_PATTERN = [
  "(?<![\\w:])(?=[\\da-f:]*\\d)(?:",
  [
    `(?:${HEX4}:){7}${HEX4}`,
    `(?:${HEX4}:){1,7}:`,
    `(?:${HEX4}:){1,6}:${HEX4}`,
    `(?:${HEX4}:){1,5}(?::${HEX4}){1,2}`,
    `(?:${HEX4}:){1,4}(?::${HEX4}){1,3}`,
    `(?:${HEX4}:){1,3}(?::${HEX4}){1,4}`,
    `(?:${HEX4}:){1,2}(?::${HEX4}){1,5}`,
    `${HEX4}:(?::${HEX4}){1,6}`,
    `:(?::${HEX4}){1,7}`,
  ].join("|"),
  ")(?:/\\d{1,3})?(?![\\w:])",
].join("")

/** Colon or dash separated pairs, and Cisco's dotted groups of four. */
const MAC_PATTERN =
  "(?<![\\w:-])(?:[\\da-f]{2}(?::[\\da-f]{2}){5}|[\\da-f]{2}(?:-[\\da-f]{2}){5}|[\\da-f]{4}\\.[\\da-f]{4}\\.[\\da-f]{4})(?![\\w:-])"

function builtinRule(
  id: string,
  pattern: string,
  foreground: HighlightPaletteColor,
  wholeWord: boolean
): HighlightRule {
  return {
    id,
    enabled: true,
    pattern,
    kind: "regex",
    case_sensitive: false,
    whole_word: wholeWord,
    foreground,
    background: null,
  }
}

/** Few rules on purpose: a screen full of color stops pointing at anything. */
export function builtinHighlightSet(): HighlightSet {
  return {
    id: BUILTIN_HIGHLIGHT_SET_ID,
    name: "",
    rules: [
      builtinRule(
        "builtin-errors",
        "errors?|fail(?:ed|ure|s)?|fatal|denied|refused|panic(?:ked)?|critical",
        "red",
        true
      ),
      builtinRule(
        "builtin-warnings",
        "warn(?:ing|ings)?|timeout|timed out|deprecated",
        "yellow",
        true
      ),
      builtinRule("builtin-success", "ok|success(?:ful|fully)?|succeeded|passed", "green", true),
      builtinRule("builtin-ipv4", IPV4_PATTERN, "cyan", false),
      builtinRule("builtin-ipv6", IPV6_PATTERN, "cyan", false),
      builtinRule("builtin-mac", MAC_PATTERN, "magenta", false),
    ],
  }
}

/** A set's name as shown: the built-in set's is translated. */
export function highlightSetName(set: HighlightSet, t: (key: string) => string): string {
  if (set.name) return set.name
  return set.id === BUILTIN_HIGHLIGHT_SET_ID
    ? t("keywordHighlight.builtinSetName")
    : t("keywordHighlight.unnamedSet")
}

export function defaultKeywordHighlightConfig(): KeywordHighlightConfig {
  return {
    enabled: true,
    default_set: BUILTIN_HIGHLIGHT_SET_ID,
    alternate_screen: false,
    sets: null,
  }
}

/** The sets in effect: the user's, or the built-in ones until they edit any. */
export function highlightSets(config: KeywordHighlightConfig): HighlightSet[] {
  return config.sets ?? [builtinHighlightSet()]
}

/**
 * The set a terminal uses, or null for none. `choice` is its connection's
 * setting; a set that was deleted falls back to the default set.
 */
export function resolveHighlightSet(
  config: KeywordHighlightConfig,
  choice: string | null | undefined
): HighlightSet | null {
  if (!config.enabled || choice === HIGHLIGHT_SET_OFF) return null
  const sets = highlightSets(config)
  const wanted = choice && choice !== HIGHLIGHT_SET_FOLLOW_DEFAULT ? choice : config.default_set
  return (
    sets.find((set) => set.id === wanted) ??
    sets.find((set) => set.id === config.default_set) ??
    sets[0] ??
    null
  )
}

/** Sets and rules past these counts are dropped when the config is read. */
export const MAX_HIGHLIGHT_SETS = 50
export const MAX_HIGHLIGHT_RULES = 100
export const MAX_HIGHLIGHT_PATTERN_LENGTH = 1000

type Fields<T> = Partial<Record<keyof T, unknown>>

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function normalizeColor(value: unknown): string | null {
  if (typeof value !== "string") return null
  const color = value.trim()
  if (HIGHLIGHT_PALETTE_COLORS.some((name) => name === color)) return color
  return /^#[\da-f]{6}$/i.test(color) ? color.toLowerCase() : null
}

function normalizeRule(value: unknown): HighlightRule | null {
  if (!isRecord(value)) return null
  const rule = value as Fields<HighlightRule>
  if (typeof rule.id !== "string" || !rule.id || typeof rule.pattern !== "string") return null
  return {
    id: rule.id,
    enabled: rule.enabled !== false,
    pattern: rule.pattern.slice(0, MAX_HIGHLIGHT_PATTERN_LENGTH),
    kind: rule.kind === "regex" ? "regex" : "keyword",
    case_sensitive: rule.case_sensitive === true,
    whole_word: rule.whole_word === true,
    foreground: normalizeColor(rule.foreground),
    background: normalizeColor(rule.background),
  }
}

function normalizeSet(value: unknown): HighlightSet | null {
  if (!isRecord(value)) return null
  const set = value as Fields<HighlightSet>
  if (typeof set.id !== "string" || !set.id) return null
  const rules = Array.isArray(set.rules) ? set.rules : []
  return {
    id: set.id,
    name: typeof set.name === "string" ? set.name : "",
    rules: rules
      .map(normalizeRule)
      .filter((rule): rule is HighlightRule => rule !== null)
      .slice(0, MAX_HIGHLIGHT_RULES),
  }
}

export function normalizeKeywordHighlightConfig(value: unknown): KeywordHighlightConfig {
  const defaults = defaultKeywordHighlightConfig()
  if (!isRecord(value)) return defaults
  const config = value as Fields<KeywordHighlightConfig>
  const sets = Array.isArray(config.sets)
    ? config.sets
        .map(normalizeSet)
        .filter((set): set is HighlightSet => set !== null)
        .slice(0, MAX_HIGHLIGHT_SETS)
    : null
  return {
    enabled: config.enabled !== false,
    default_set:
      typeof config.default_set === "string" && config.default_set
        ? config.default_set
        : defaults.default_set,
    alternate_screen: config.alternate_screen === true,
    sets,
  }
}
