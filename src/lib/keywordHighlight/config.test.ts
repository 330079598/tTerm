import { describe, expect, it } from "vitest"

import {
  BUILTIN_HIGHLIGHT_SET_ID,
  defaultKeywordHighlightConfig,
  HIGHLIGHT_SET_FOLLOW_DEFAULT,
  HIGHLIGHT_SET_OFF,
  type HighlightSet,
  MAX_HIGHLIGHT_RULES,
  normalizeKeywordHighlightConfig,
  resolveHighlightSet,
} from "@/lib/keywordHighlight/config"

const network: HighlightSet = { id: "network", name: "Network", rules: [] }
const servers: HighlightSet = { id: "servers", name: "Servers", rules: [] }

describe("resolveHighlightSet", () => {
  it("uses the built-in set until the user has sets of their own", () => {
    const set = resolveHighlightSet(defaultKeywordHighlightConfig(), undefined)
    expect(set?.id).toBe(BUILTIN_HIGHLIGHT_SET_ID)
  })

  it("follows the default set unless the connection picks one or none", () => {
    const config = {
      ...defaultKeywordHighlightConfig(),
      default_set: "servers",
      sets: [network, servers],
    }
    expect(resolveHighlightSet(config, HIGHLIGHT_SET_FOLLOW_DEFAULT)).toBe(servers)
    expect(resolveHighlightSet(config, null)).toBe(servers)
    expect(resolveHighlightSet(config, "network")).toBe(network)
    expect(resolveHighlightSet(config, HIGHLIGHT_SET_OFF)).toBeNull()
  })

  it("falls back to the default set when the chosen one was deleted", () => {
    const config = { ...defaultKeywordHighlightConfig(), default_set: "servers", sets: [servers] }
    expect(resolveHighlightSet(config, "network")).toBe(servers)
  })

  it("falls back to the first set when the default one was deleted", () => {
    const config = { ...defaultKeywordHighlightConfig(), default_set: "gone", sets: [network] }
    expect(resolveHighlightSet(config, undefined)).toBe(network)
  })

  it("highlights nothing while turned off", () => {
    const config = { ...defaultKeywordHighlightConfig(), enabled: false }
    expect(resolveHighlightSet(config, "network")).toBeNull()
  })
})

describe("normalizeKeywordHighlightConfig", () => {
  it("defaults to the built-in set, on, and off in full-screen programs", () => {
    expect(normalizeKeywordHighlightConfig(undefined)).toEqual(defaultKeywordHighlightConfig())
    expect(normalizeKeywordHighlightConfig({ sets: "nonsense" }).sets).toBeNull()
  })

  it("keeps valid rules and drops what it cannot read", () => {
    const config = normalizeKeywordHighlightConfig({
      enabled: false,
      default_set: "net",
      alternate_screen: true,
      sets: [
        {
          id: "net",
          name: "Network",
          rules: [
            {
              id: "a",
              pattern: "down",
              kind: "regex",
              foreground: "brightRed",
              background: "#ABCDEF",
            },
            { id: "b", pattern: "up", foreground: "rgb(1, 2, 3)" },
            { pattern: "no id" },
            "nonsense",
          ],
        },
        { name: "no id" },
      ],
    })
    expect(config.enabled).toBe(false)
    expect(config.alternate_screen).toBe(true)
    expect(config.sets).toEqual([
      {
        id: "net",
        name: "Network",
        rules: [
          {
            id: "a",
            enabled: true,
            pattern: "down",
            kind: "regex",
            case_sensitive: false,
            whole_word: false,
            foreground: "brightRed",
            background: "#abcdef",
          },
          {
            id: "b",
            enabled: true,
            pattern: "up",
            kind: "keyword",
            case_sensitive: false,
            whole_word: false,
            foreground: null,
            background: null,
          },
        ],
      },
    ])
  })

  it("caps the rules of a set", () => {
    const rules = Array.from({ length: MAX_HIGHLIGHT_RULES + 5 }, (_, index) => ({
      id: `r${index}`,
      pattern: "x",
    }))
    const config = normalizeKeywordHighlightConfig({ sets: [{ id: "s", rules }] })
    expect(config.sets?.[0].rules).toHaveLength(MAX_HIGHLIGHT_RULES)
  })
})
