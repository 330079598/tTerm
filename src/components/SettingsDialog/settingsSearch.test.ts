// @vitest-environment jsdom

import { afterEach, describe, expect, it, vi } from "vitest"

import en from "@/i18n/locales/en.json"
import zh from "@/i18n/locales/zh.json"
import {
  buildSettingsSearchItems,
  revealSetting,
  searchSettings,
} from "@/components/SettingsDialog/settingsSearch"
import { SETTINGS_SEARCH_INDEX } from "@/components/SettingsDialog/settingsSearchIndex"
import { SETTINGS_TABS } from "@/components/SettingsDialog/settingsTabs"

type Messages = Record<string, unknown>

function lookup(messages: Messages, key: string) {
  const value = key
    .split(".")
    .reduce<unknown>((node, part) => (node as Messages | undefined)?.[part], messages)
  return typeof value === "string" ? value : undefined
}

const translator = (messages: Messages) => (key: string, options?: { defaultValue?: string }) =>
  lookup(messages, key) ?? options?.defaultValue ?? key

const titles = (results: { title: string }[]) => results.map((result) => result.title)

afterEach(() => {
  document.body.innerHTML = ""
  // Undoes the stand-in for Web Animations that a test may install.
  Reflect.deleteProperty(HTMLElement.prototype, "animate")
})

describe("settings search index", () => {
  it("lists only pages the sidebar has", () => {
    const tabs = new Set(SETTINGS_TABS.map((tab) => tab.value))
    expect(Object.keys(SETTINGS_SEARCH_INDEX).filter((tab) => !tabs.has(tab))).toEqual([])
  })

  it("names only keys both languages translate", () => {
    const keys = Object.values(SETTINGS_SEARCH_INDEX)
      .flat()
      .flatMap((entry) => [entry.title, entry.description ?? entry.title])
    for (const messages of [en, zh]) {
      expect(keys.filter((key) => lookup(messages, key) === undefined)).toEqual([])
    }
  })
})

describe("searchSettings", () => {
  const items = buildSettingsSearchItems(translator(en))

  it("returns nothing for an empty query", () => {
    expect(searchSettings(items, "   ")).toEqual([])
  })

  it("finds a setting by its title, ignoring case", () => {
    const results = searchSettings(items, "FONT SIZE")
    expect(results[0]).toMatchObject({ tab: "terminal", title: "Font Size", isPage: false })
  })

  it("finds a setting by words from its description", () => {
    const description = lookup(en, "settings.sftpPasteUploadDesc") ?? ""
    const word = description.split(/\s+/).find((part) => part.length > 6) ?? ""
    expect(titles(searchSettings(items, word))).toContain(lookup(en, "settings.sftpPasteUpload"))
  })

  it("does not list a whole page for a word only its name has", () => {
    const page = { tab: "updates", tabLabel: "Updates", isPage: true }
    const pageItems = [
      { ...page, title: "Updates" },
      { ...page, title: "Update channel", isPage: false },
    ]
    expect(titles(searchSettings(pageItems, "updates"))).toEqual(["Updates"])
  })

  it("matches words from the page name together with the setting's own", () => {
    const results = searchSettings(items, "terminal font")
    expect(titles(results)).toContain("Font Size")
  })

  it("ranks titles that start with the query first", () => {
    const results = searchSettings(items, "font")
    expect(results[0].title.toLocaleLowerCase().startsWith("font")).toBe(true)
  })

  it("searches the current language", () => {
    const results = searchSettings(buildSettingsSearchItems(translator(zh)), "字体大小")
    expect(results.length).toBeGreaterThan(0)
    expect(results[0].tab).toBe("terminal")
  })
})

describe("revealSetting", () => {
  // jsdom has neither layout nor Web Animations, so tests give elements a
  // box and record the frame's animation.
  const animations: Array<{ onfinish: (() => void) | null }> = []

  function installAnimate() {
    HTMLElement.prototype.animate = vi.fn(() => {
      const animation = { onfinish: null }
      animations.push(animation)
      return animation as unknown as Animation
    })
  }

  function placeAt(id: string, left: number, top: number, width: number, height: number) {
    const element = document.getElementById(id) as HTMLElement
    const box = { left, top, width, height, right: left + width, bottom: top + height }
    element.getBoundingClientRect = () => box as DOMRect
  }

  /** Renders a page inside its host, as the settings panel has them. */
  function renderPage(content: string) {
    document.body.innerHTML = `<div id="host"><div id="panel">${content}</div></div>`
    placeAt("host", 0, 0, 800, 600)
    placeAt("panel", 100, 50, 600, 500)
    return document.getElementById("panel") as HTMLElement
  }

  const frames = () => document.querySelectorAll<HTMLElement>("[data-settings-search-frame]")

  afterEach(() => {
    animations.length = 0
  })

  it("frames the card titled like the setting without taking the focus", () => {
    const panel = renderPage(`
      <div data-slot="card" id="other"><div>Other</div><button></button></div>
      <div data-slot="card" id="wanted"><div>Copy on select</div><button></button></div>`)
    installAnimate()
    placeAt("wanted", 130, 200, 400, 60)

    expect(revealSetting(panel, "Copy on select")).toBe(true)
    const [frame] = frames()
    expect(frame.parentElement?.id).toBe("host")
    expect(frame.style).toMatchObject({
      left: "126px",
      top: "196px",
      width: "408px",
      height: "68px",
    })
    expect(document.activeElement).toBe(document.body)
  })

  it("leaves room around a section and is cut to the page", () => {
    const panel = renderPage(`<section id="section"><h3>Startup</h3><button></button></section>`)
    installAnimate()
    placeAt("section", 124, 52, 500, 80)

    revealSetting(panel, "Startup")
    const [frame] = frames()
    expect(frame.style).toMatchObject({ left: "116px", top: "44px", width: "516px" })
    // The frame starts above the page's top edge, so that part is cut off.
    expect(frame.style.clipPath).toBe("inset(6px 0px 0px 0px)")
  })

  it("shows one frame at a time and takes it down when it fades", () => {
    const panel = renderPage(`<div data-slot="card" id="card"><div>Copy on select</div></div>`)
    installAnimate()
    placeAt("card", 130, 100, 10, 10)

    revealSetting(panel, "Copy on select")
    revealSetting(panel, "Copy on select")
    const host = document.getElementById("host") as HTMLElement
    expect(frames()).toHaveLength(1)
    expect(host.style.position).toBe("relative")

    animations[animations.length - 1].onfinish?.()
    expect(frames()).toHaveLength(0)
    expect(host.style.position).toBe("")
  })

  it("reports a setting the page has not rendered", () => {
    document.body.innerHTML = `<div id="panel"><select><option>Copy on select</option></select></div>`
    const panel = document.getElementById("panel") as HTMLElement

    expect(revealSetting(panel, "Copy on select")).toBe(false)
  })
})
