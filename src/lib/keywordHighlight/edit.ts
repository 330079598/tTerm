// Edits to the keyword highlight config, as the settings page makes them.
// The first edit turns the built-in set into one of the user's own, which
// is saved from then on.
import {
  BUILTIN_HIGHLIGHT_SET_ID,
  builtinHighlightSet,
  highlightSets,
  type HighlightRule,
  type HighlightSet,
  type KeywordHighlightConfig,
  MAX_HIGHLIGHT_RULES,
  MAX_HIGHLIGHT_SETS,
} from "@/lib/keywordHighlight/config"

export function newHighlightId(): string {
  return crypto.randomUUID()
}

function withSets(config: KeywordHighlightConfig, sets: HighlightSet[]): KeywordHighlightConfig {
  return { ...config, sets }
}

/** Changes one set, leaving the config as it is when there is no such set. */
export function updateHighlightSet(
  config: KeywordHighlightConfig,
  setId: string,
  change: (set: HighlightSet) => HighlightSet
): KeywordHighlightConfig {
  const sets = highlightSets(config)
  if (!sets.some((set) => set.id === setId)) return config
  return withSets(
    config,
    sets.map((set) => (set.id === setId ? change(set) : set))
  )
}

/**
 * Adds a set, empty or a copy of `source`'s rules, and returns its id; null
 * when there are as many sets as allowed.
 */
export function addHighlightSet(
  config: KeywordHighlightConfig,
  name: string,
  source?: HighlightSet
): { config: KeywordHighlightConfig; id: string } | null {
  const sets = highlightSets(config)
  if (sets.length >= MAX_HIGHLIGHT_SETS) return null
  const id = newHighlightId()
  const rules = source?.rules.map((rule) => ({ ...rule, id: newHighlightId() })) ?? []
  return { config: withSets(config, [...sets, { id, name, rules }]), id }
}

/** Removes a set but never the last one; the default moves to the first left. */
export function deleteHighlightSet(
  config: KeywordHighlightConfig,
  setId: string
): KeywordHighlightConfig {
  const sets = highlightSets(config)
  const remaining = sets.filter((set) => set.id !== setId)
  if (remaining.length === 0 || remaining.length === sets.length) return config
  const defaultSet = remaining.some((set) => set.id === config.default_set)
    ? config.default_set
    : remaining[0].id
  return { ...config, default_set: defaultSet, sets: remaining }
}

/** Puts the built-in rules back into the built-in set. */
export function restoreBuiltinRules(set: HighlightSet): HighlightSet {
  return set.id === BUILTIN_HIGHLIGHT_SET_ID ? { ...set, rules: builtinHighlightSet().rules } : set
}

/** A new rule for the user to fill in: a whole-word keyword in red. */
export function newHighlightRule(): HighlightRule {
  return {
    id: newHighlightId(),
    enabled: true,
    pattern: "",
    kind: "keyword",
    case_sensitive: false,
    whole_word: true,
    foreground: "red",
    background: null,
  }
}

export function addHighlightRule(set: HighlightSet, rule: HighlightRule): HighlightSet {
  if (set.rules.length >= MAX_HIGHLIGHT_RULES) return set
  return { ...set, rules: [...set.rules, rule] }
}

export function updateHighlightRule(
  set: HighlightSet,
  ruleId: string,
  changes: Partial<Omit<HighlightRule, "id">>
): HighlightSet {
  return {
    ...set,
    rules: set.rules.map((rule) => (rule.id === ruleId ? { ...rule, ...changes } : rule)),
  }
}

export function removeHighlightRule(set: HighlightSet, ruleId: string): HighlightSet {
  return { ...set, rules: set.rules.filter((rule) => rule.id !== ruleId) }
}

/** Moves a rule up (-1) or down (1); earlier rules win where matches overlap. */
export function moveHighlightRule(set: HighlightSet, ruleId: string, offset: -1 | 1): HighlightSet {
  const index = set.rules.findIndex((rule) => rule.id === ruleId)
  const target = index + offset
  if (index < 0 || target < 0 || target >= set.rules.length) return set
  const rules = [...set.rules]
  ;[rules[index], rules[target]] = [rules[target], rules[index]]
  return { ...set, rules }
}
