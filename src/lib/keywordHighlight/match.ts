import { colorToHex } from "@/components/ThemeEditor/colorUtils"
import {
  HIGHLIGHT_PALETTE_COLORS,
  type HighlightPaletteColor,
  type HighlightRule,
  type HighlightSet,
} from "@/lib/keywordHighlight/config"

/** The terminal theme's ANSI colors; xterm's own stand in for missing ones. */
export type HighlightPalette = Partial<Record<HighlightPaletteColor, string>>

/** xterm's colors for a theme that leaves them unset. */
const XTERM_DEFAULT_COLORS: Record<HighlightPaletteColor, string> = {
  black: "#2e3436",
  red: "#cc0000",
  green: "#4e9a06",
  yellow: "#c4a000",
  blue: "#3465a4",
  magenta: "#75507b",
  cyan: "#06989a",
  white: "#d3d7cf",
  brightBlack: "#555753",
  brightRed: "#ef2929",
  brightGreen: "#8ae234",
  brightYellow: "#fce94f",
  brightBlue: "#729fcf",
  brightMagenta: "#ad7fa8",
  brightCyan: "#34e2e2",
  brightWhite: "#eeeeec",
}

/** A rule ready to run, its colors resolved to the `#rrggbb` xterm requires. */
export interface CompiledHighlightRule {
  regex: RegExp
  foreground?: string
  background?: string
}

/** Where a rule matched, in UTF-16 offsets of the text matched. */
export interface HighlightSpan {
  start: number
  end: number
  foreground?: string
  background?: string
}

/**
 * A pathological pattern can take very long on a long line, and JavaScript
 * cannot interrupt a regex; only this much of a line is matched.
 */
export const MAX_HIGHLIGHT_TEXT = 2000
/** Past this many highlights a line is mostly color, and more would only cost. */
export const MAX_SPANS_PER_LINE = 20

// Letters, digits and underscore, as `\w` means without the `u` flag. Text in
// scripts without word spaces, such as Chinese, counts as a boundary.
const WORD_START = "(?<!\\w)"
const WORD_END = "(?!\\w)"

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")
}

function ruleSource(rule: Pick<HighlightRule, "pattern" | "kind" | "whole_word">): string {
  const body = rule.kind === "keyword" ? escapeRegExp(rule.pattern) : rule.pattern
  return rule.whole_word ? `${WORD_START}(?:${body})${WORD_END}` : body
}

function ruleFlags(rule: Pick<HighlightRule, "case_sensitive">): string {
  return rule.case_sensitive ? "g" : "gi"
}

/**
 * Why a rule's pattern cannot be used, or null when it can. A pattern that
 * matches empty text would color nothing and is refused too.
 */
export function highlightPatternError(
  rule: Pick<HighlightRule, "pattern" | "kind" | "whole_word" | "case_sensitive">
): "empty" | "invalid" | "matchesEmpty" | null {
  if (!rule.pattern.trim()) return "empty"
  let regex: RegExp
  try {
    regex = new RegExp(ruleSource(rule), ruleFlags(rule))
  } catch {
    return "invalid"
  }
  return regex.test("") ? "matchesEmpty" : null
}

/** A palette color name or a hex color as `#rrggbb`, or undefined for none. */
export function resolveHighlightColor(
  color: string | null | undefined,
  palette: HighlightPalette
): string | undefined {
  if (!color) return undefined
  const named = HIGHLIGHT_PALETTE_COLORS.find((name) => name === color)
  if (!named) return colorToHex(color) ?? undefined
  return colorToHex(palette[named] ?? "") ?? XTERM_DEFAULT_COLORS[named]
}

/** The set's enabled, valid rules that set a color, in the set's order. */
export function compileHighlightSet(
  set: HighlightSet,
  palette: HighlightPalette
): CompiledHighlightRule[] {
  const compiled: CompiledHighlightRule[] = []
  for (const rule of set.rules) {
    if (!rule.enabled || highlightPatternError(rule)) continue
    const foreground = resolveHighlightColor(rule.foreground, palette)
    const background = resolveHighlightColor(rule.background, palette)
    if (!foreground && !background) continue
    compiled.push({ regex: new RegExp(ruleSource(rule), ruleFlags(rule)), foreground, background })
  }
  return compiled
}

/**
 * Highlights in `text`, sorted by position and never overlapping. Where
 * matches overlap, the rule earlier in the set wins.
 */
export function matchHighlights(
  text: string,
  rules: readonly CompiledHighlightRule[]
): HighlightSpan[] {
  const input = text.length > MAX_HIGHLIGHT_TEXT ? text.slice(0, MAX_HIGHLIGHT_TEXT) : text
  const accepted: HighlightSpan[] = []
  for (const rule of rules) {
    if (accepted.length >= MAX_SPANS_PER_LINE) break
    rule.regex.lastIndex = 0
    for (const match of input.matchAll(rule.regex)) {
      const start = match.index
      const end = start + match[0].length
      if (end === start) continue
      if (accepted.some((span) => start < span.end && span.start < end)) continue
      accepted.push({ start, end, foreground: rule.foreground, background: rule.background })
      if (accepted.length >= MAX_SPANS_PER_LINE) break
    }
  }
  return accepted.sort((a, b) => a.start - b.start)
}
