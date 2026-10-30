import { SETTINGS_SEARCH_INDEX } from "@/components/SettingsDialog/settingsSearchIndex"
import { SETTINGS_TABS } from "@/components/SettingsDialog/settingsTabs"

type Translate = (key: string, options?: { defaultValue?: string }) => string

/** The id of the results list, which the search field controls. */
export const SETTINGS_SEARCH_RESULTS_ID = "settings-search-results"

/** The id of one result, so the field can point at the highlighted one. */
export const settingsSearchResultId = (index: number) => `settings-search-result-${index}`

/** A setting, or a whole page, that matches the search. */
export interface SettingsSearchResult {
  tab: string
  tabLabel: string
  title: string
  description?: string
  /** The result is the page itself rather than a setting on it. */
  isPage: boolean
}

function normalize(text: string) {
  return text.normalize("NFKC").toLocaleLowerCase().trim()
}

/** Lists every page and setting with its text in the current language. */
export function buildSettingsSearchItems(t: Translate): SettingsSearchResult[] {
  return SETTINGS_TABS.flatMap((tab) => {
    const tabLabel = t(tab.labelKey, { defaultValue: tab.defaultLabel })
    const entries = (SETTINGS_SEARCH_INDEX[tab.value] ?? []).map((entry) => ({
      tab: tab.value,
      tabLabel,
      title: t(entry.title),
      description: entry.description ? t(entry.description) : undefined,
      isPage: false,
    }))
    return [{ tab: tab.value, tabLabel, title: tabLabel, isPage: true }, ...entries]
  })
}

/**
 * Ranks how well an item matches. Every word of the query has to appear in
 * its title, description or page name, and at least one of them in its own
 * title or description, so a page name alone does not list the whole page.
 */
function scoreItem(item: SettingsSearchResult, query: string, words: string[]) {
  const title = normalize(item.title)
  const ownText = `${title} ${normalize(item.description ?? "")}`
  const allText = `${ownText} ${normalize(item.tabLabel)}`

  if (!words.every((word) => allText.includes(word))) return 0
  if (!words.some((word) => ownText.includes(word))) return 0
  if (title === query) return 5
  if (title.startsWith(query)) return 4
  if (title.includes(query)) return 3
  if (words.every((word) => title.includes(word))) return 2
  return 1
}

/** Finds the settings that match the query, best matches first. */
export function searchSettings(items: SettingsSearchResult[], query: string) {
  const normalized = normalize(query)
  if (!normalized) return []

  const words = normalized.split(/\s+/)
  return items
    .map((item, index) => ({ item, index, score: scoreItem(item, normalized, words) }))
    .filter((match) => match.score > 0)
    .sort((a, b) => b.score - a.score || a.index - b.index)
    .map((match) => match.item)
}

const CONTROLS = "input, select, textarea, button, [tabindex]:not([tabindex='-1'])"

/** The nearest element around a label that also holds the control it names. */
function fieldAround(label: HTMLElement, panel: HTMLElement) {
  for (let element = label.parentElement; element && element !== panel;) {
    if (element.querySelector(CONTROLS)) return element
    element = element.parentElement
  }
  return label.parentElement ?? label
}

/** How long a revealed setting stays marked, fading out over the second half. */
const REVEAL_MARK_MS = 1600

/**
 * Marks an element for a moment by animating one of its colors from `color`
 * back to its own. The last keyframe is left out, so the animation ends on
 * whatever the element's styles give it.
 */
function markBriefly(element: HTMLElement, property: "backgroundColor" | "color", color: string) {
  element.animate?.(
    [
      { [property]: color, offset: 0 },
      { [property]: color, offset: 0.5 },
    ],
    { duration: REVEAL_MARK_MS, easing: "ease-out" }
  )
}

/**
 * Finds the setting titled `title` on a settings page, scrolls it into view
 * and briefly marks it. Returns false when the page has not rendered it,
 * which happens while the page loads or when the setting is on a part of the
 * page that is hidden right now.
 */
export function revealSetting(panel: HTMLElement, title: string) {
  const wanted = title.trim()
  const walker = document.createTreeWalker(panel, NodeFilter.SHOW_TEXT)
  let label: HTMLElement | null = null
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    const parent = node.parentElement
    if (node.textContent?.trim() === wanted && parent && parent.tagName !== "OPTION") {
      label = parent
      break
    }
  }
  if (!label) return false

  // A section heading stands for its whole section and a row title for its
  // card; a bare label stands for the field it sits with.
  const card = label.closest<HTMLElement>('[data-slot="card"]')
  const target =
    card ??
    (label.tagName === "H3" ? label.closest<HTMLElement>("section") : null) ??
    fieldAround(label, panel)

  target.scrollIntoView?.({ block: "center", behavior: "smooth" })
  // The mark is a tint and a colored title, not an outline: a section or a
  // field has no padding, so a line drawn on its edge would cut through its
  // text, and the page's scroll area would clip one drawn outside it.
  if (card) markBriefly(card, "backgroundColor", "hsl(var(--primary) / 0.12)")
  markBriefly(label, "color", "hsl(var(--primary))")
  return true
}
