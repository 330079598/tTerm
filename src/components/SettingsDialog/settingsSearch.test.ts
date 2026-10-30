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
  it("marks the card titled like the setting without taking the focus", () => {
    document.body.innerHTML = `
      <div id="panel">
        <div data-slot="card" id="other"><div>Other</div><button></button></div>
        <div data-slot="card" id="wanted"><div>Copy on select</div><button></button></div>
      </div>`
    const panel = document.getElementById("panel") as HTMLElement
    const marked: string[] = []
    // jsdom has no Web Animations, so the mark is recorded instead.
    HTMLElement.prototype.animate = vi.fn(function (this: HTMLElement, keyframes) {
      const [first] = keyframes as Keyframe[]
      if (first.backgroundColor) marked.push(this.id)
      return {} as Animation
    })

    expect(revealSetting(panel, "Copy on select")).toBe(true)
    expect(marked).toEqual(["wanted"])
    expect(document.activeElement).toBe(document.body)
  })

  it("reports a setting the page has not rendered", () => {
    document.body.innerHTML = `<div id="panel"><select><option>Copy on select</option></select></div>`
    const panel = document.getElementById("panel") as HTMLElement

    expect(revealSetting(panel, "Copy on select")).toBe(false)
  })
})
