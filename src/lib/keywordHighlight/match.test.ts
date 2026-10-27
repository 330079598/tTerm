import { describe, expect, it } from "vitest"

import {
  builtinHighlightSet,
  type HighlightRule,
  type HighlightSet,
} from "@/lib/keywordHighlight/config"
import {
  compileHighlightSet,
  highlightPatternError,
  matchHighlights,
  MAX_HIGHLIGHT_TEXT,
  MAX_SPANS_PER_LINE,
  resolveHighlightColor,
} from "@/lib/keywordHighlight/match"
import type { TerminalPalette } from "@/types/theme"

const PALETTE: TerminalPalette = {
  background: "#000000",
  foreground: "#ffffff",
  cursor: "#ffffff",
  selectionBackground: "#444444",
  black: "#000000",
  red: "#cc0000",
  green: "#00cc00",
  yellow: "#cccc00",
  blue: "#0000cc",
  magenta: "#cc00cc",
  cyan: "#00cccc",
  white: "#cccccc",
  brightBlack: "#555555",
  brightRed: "#ff5555",
  brightGreen: "#55ff55",
  brightYellow: "#ffff55",
  brightBlue: "#5555ff",
  brightMagenta: "#ff55ff",
  brightCyan: "#55ffff",
  brightWhite: "#ffffff",
}

function rule(overrides: Partial<HighlightRule>): HighlightRule {
  return {
    id: "rule",
    enabled: true,
    pattern: "error",
    kind: "keyword",
    case_sensitive: false,
    whole_word: false,
    foreground: "red",
    background: null,
    ...overrides,
  }
}

function setOf(...rules: HighlightRule[]): HighlightSet {
  return { id: "set", name: "Set", rules }
}

/** The matched texts, for reading tests at a glance. */
function matched(text: string, set: HighlightSet): string[] {
  return matchHighlights(text, compileHighlightSet(set, PALETTE)).map((span) =>
    text.slice(span.start, span.end)
  )
}

const builtinMatches = (text: string) => matched(text, builtinHighlightSet())

describe("built-in highlight set", () => {
  it("colors errors, warnings and successes as whole words in any case", () => {
    expect(builtinMatches("ERROR: connection refused, Failed twice")).toEqual([
      "ERROR",
      "refused",
      "Failed",
    ])
    expect(builtinMatches("warning: request timed out")).toEqual(["warning", "timed out"])
    // "ok" is too common to color: it would light up most command output.
    expect(builtinMatches("[ OK ] tests passed")).toEqual(["passed"])
    expect(builtinMatches("errorlevel token failsafe")).toEqual([])
  })

  it("colors errors next to Chinese text, which has no spaces between words", () => {
    expect(builtinMatches("发生error了")).toEqual(["error"])
  })

  it("colors IPv4 addresses with a prefix length, not longer dotted numbers", () => {
    expect(builtinMatches("from 192.168.1.10 via 10.0.0.0/8")).toEqual([
      "192.168.1.10",
      "10.0.0.0/8",
    ])
    expect(builtinMatches("version 1.2.3.4.5 and 256.1.1.1")).toEqual([])
  })

  it("colors IPv6 addresses but not times or hex words around double colons", () => {
    expect(builtinMatches("inet6 fe80::1/64 and 2001:db8:0:0:0:0:2:1 and ::1")).toEqual([
      "fe80::1/64",
      "2001:db8:0:0:0:0:2:1",
      "::1",
    ])
    expect(builtinMatches("at 12:30:45 Add::add std::cafe")).toEqual([])
  })

  it("colors MAC addresses in the usual notations", () => {
    expect(builtinMatches("ether 52:54:00:ab:cd:ef 52-54-00-AB-CD-EF 5254.00ab.cdef")).toEqual([
      "52:54:00:ab:cd:ef",
      "52-54-00-AB-CD-EF",
      "5254.00ab.cdef",
    ])
  })
})

describe("matchHighlights", () => {
  it("matches keywords literally, regex characters included", () => {
    expect(matched("a.b axb", setOf(rule({ pattern: "a.b" })))).toEqual(["a.b"])
  })

  it("respects case sensitivity and whole words", () => {
    const set = setOf(rule({ pattern: "Error", case_sensitive: true, whole_word: true }))
    expect(matched("error Error Errors", set)).toEqual(["Error"])
  })

  it("lets the earlier rule win where matches overlap", () => {
    const set = setOf(
      rule({ id: "a", pattern: "denied", foreground: "red" }),
      rule({ id: "b", kind: "regex", pattern: "permission \\w+", foreground: "green" })
    )
    const spans = matchHighlights(
      "permission denied, permission granted",
      compileHighlightSet(set, PALETTE)
    )
    expect(spans).toEqual([
      { start: 11, end: 17, foreground: "#cc0000", background: undefined },
      { start: 19, end: 37, foreground: "#00cc00", background: undefined },
    ])
  })

  it("skips disabled, invalid and colorless rules", () => {
    const set = setOf(
      rule({ pattern: "one", enabled: false }),
      rule({ pattern: "(two", kind: "regex" }),
      rule({ pattern: "three", foreground: null, background: null }),
      rule({ pattern: "four", foreground: null, background: "#123456" })
    )
    expect(matched("one two three four", set)).toEqual(["four"])
  })

  it("caps the highlights of a line and the text it matches", () => {
    const set = setOf(rule({ pattern: "x" }))
    expect(matched("x ".repeat(100), set)).toHaveLength(MAX_SPANS_PER_LINE)
    expect(matched(`${" ".repeat(MAX_HIGHLIGHT_TEXT)}x`, set)).toEqual([])
  })
})

describe("highlightPatternError", () => {
  it("refuses empty, invalid and empty-matching patterns", () => {
    expect(highlightPatternError(rule({ pattern: "  " }))).toBe("empty")
    expect(highlightPatternError(rule({ pattern: "[a", kind: "regex" }))).toBe("invalid")
    expect(highlightPatternError(rule({ pattern: "a*", kind: "regex" }))).toBe("matchesEmpty")
    expect(highlightPatternError(rule({ pattern: "[a", kind: "keyword" }))).toBeNull()
  })
})

describe("resolveHighlightColor", () => {
  it("takes palette names from the theme and normalizes hex colors", () => {
    expect(resolveHighlightColor("brightRed", PALETTE)).toBe("#ff5555")
    expect(resolveHighlightColor("#ABC", PALETTE)).toBe("#aabbcc")
    expect(resolveHighlightColor("nonsense", PALETTE)).toBeUndefined()
    expect(resolveHighlightColor(null, PALETTE)).toBeUndefined()
  })

  it("uses xterm's colors for those the theme leaves unset", () => {
    expect(resolveHighlightColor("red", {})).toBe("#cc0000")
  })
})
