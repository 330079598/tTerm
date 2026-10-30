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

/** How long the frame around a revealed setting shows, fading over the last part. */
const REVEAL_FRAME_MS = 1800

/** Room between the frame and a section or field, which have no padding of their own. */
const REVEAL_FRAME_GAP_PX = 8

/** Room between the frame and a card, whose padding already keeps its text clear. */
const REVEAL_FRAME_CARD_GAP_PX = 4

/** Takes down the frame on show, if any, so a new reveal never stacks two. */
let clearFrame: (() => void) | null = null

/**
 * Draws a rounded frame around the target for a moment. The frame is an
 * element of its own rather than the target's border or outline, as a line
 * on the edge of a section or field would run through its text. It sits
 * beside the page instead of inside the page's scroll area: some scroll
 * areas start right at their content, and would cut off or crowd a frame
 * that needs room around it, while the page around them has padding to
 * spare. So the frame follows the target on every animation frame while
 * the page scrolls to it, cut to the page so it never covers the sidebar
 * or the header.
 */
function frameBriefly(target: HTMLElement, panel: HTMLElement, isCard: boolean) {
  if (typeof target.animate !== "function") return
  clearFrame?.()

  const host = panel.parentElement ?? panel
  const gap = isCard ? REVEAL_FRAME_CARD_GAP_PX : REVEAL_FRAME_GAP_PX
  const radius = isCard ? (parseFloat(getComputedStyle(target).borderRadius) || 12) + gap : 12

  const frame = document.createElement("div")
  frame.setAttribute("aria-hidden", "true")
  frame.dataset.settingsSearchFrame = ""
  Object.assign(frame.style, {
    position: "absolute",
    border: "2px solid hsl(var(--ring))",
    borderRadius: `${radius}px`,
    boxSizing: "border-box",
    pointerEvents: "none",
    zIndex: "1",
  })

  const place = () => {
    const box = host.getBoundingClientRect()
    const page = panel.getBoundingClientRect()
    const rect = target.getBoundingClientRect()
    const left = rect.left - gap
    const top = rect.top - gap
    const right = rect.right + gap
    const bottom = rect.bottom + gap
    Object.assign(frame.style, {
      left: `${left - box.left - host.clientLeft}px`,
      top: `${top - box.top - host.clientTop}px`,
      width: `${right - left}px`,
      height: `${bottom - top}px`,
      clipPath: `inset(${Math.max(page.top - top, 0)}px ${Math.max(right - page.right, 0)}px ${Math.max(bottom - page.bottom, 0)}px ${Math.max(page.left - left, 0)}px)`,
    })
  }

  // The frame is placed against the host, which needs a position for that;
  // it gets one only while the frame shows.
  const restorePosition = host.style.position
  if (getComputedStyle(host).position === "static") host.style.position = "relative"
  place()
  host.appendChild(frame)

  let handle = 0
  const follow = () => {
    place()
    handle = requestAnimationFrame(follow)
  }
  handle = requestAnimationFrame(follow)

  const animation = frame.animate(
    [
      { opacity: 1, offset: 0 },
      { opacity: 1, offset: 0.6 },
      { opacity: 0, offset: 1 },
    ],
    { duration: REVEAL_FRAME_MS, easing: "ease-out" }
  )
  const cleanUp = () => {
    if (clearFrame !== cleanUp) return
    clearFrame = null
    cancelAnimationFrame(handle)
    frame.remove()
    host.style.position = restorePosition
  }
  clearFrame = cleanUp
  animation.onfinish = cleanUp
}

/**
 * Finds the setting titled `title` on a settings page, scrolls it into view
 * and briefly frames it. Returns false when the page has not rendered it,
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

  frameBriefly(target, panel, card !== null)
  target.scrollIntoView?.({ block: "center", behavior: "smooth" })
  return true
}
