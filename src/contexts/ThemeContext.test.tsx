// @vitest-environment jsdom

import { act, cleanup, renderHook, waitFor } from "@testing-library/react"
import { useSyncExternalStore } from "react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { ThemeProvider, useTheme } from "@/contexts/ThemeContext"
import type { AppConfig } from "@/contexts/ConfigContext"
import { loadThemeCatalog } from "@/lib/themeCatalog"
import { deriveThemeColors, terminalPaletteFromScheme } from "@/lib/themeDerivation"
import type { CustomTheme } from "@/types/theme"

const backend = vi.hoisted(() => ({ themes: [] as unknown[] }))

vi.mock("@tauri-apps/api/core", () => ({
  isTauri: () => false,
  invoke: vi.fn(async (command: string, args?: { themes?: unknown[] }) => {
    if (command === "load_custom_themes") return backend.themes
    if (command === "save_custom_themes") backend.themes = args?.themes ?? []
  }),
}))
vi.mock("@tauri-apps/api/webviewWindow", () => ({ getCurrentWebviewWindow: () => ({}) }))
vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? key,
  }),
}))
vi.mock("@/lib/sync", () => ({ onSyncApplied: () => () => {} }))
vi.mock("@/lib/themeCatalog", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/themeCatalog")>()
  return { ...actual, loadThemeCatalog: vi.fn(actual.loadThemeCatalog) }
})

type ThemeSettings = Pick<
  AppConfig,
  "theme" | "theme_follow_system" | "theme_light" | "theme_dark" | "favorite_themes"
>

const DEFAULT_SETTINGS: ThemeSettings = {
  theme: "default",
  theme_follow_system: false,
  theme_light: "light",
  theme_dark: "default",
  favorite_themes: [],
}

/** The saved settings; while `holdSaves`, a save lands when `releaseSave` is called. */
const config = vi.hoisted(() => ({
  current: {} as ThemeSettings,
  systemDark: false,
  listeners: new Set<() => void>(),
  releaseSave: null as null | (() => void),
  holdSaves: false,
}))

function subscribe(listener: () => void) {
  config.listeners.add(listener)
  return () => config.listeners.delete(listener)
}

function notify() {
  config.listeners.forEach((listener) => listener())
}

vi.mock("@/hooks/useSystemPrefersDark", () => ({
  useSystemPrefersDark: () => useSyncExternalStore(subscribe, () => config.systemDark),
}))

vi.mock("@/contexts/ConfigContext", () => ({
  getDetectedPlatform: () => "linux",
  isWindowBlurSupported: () => false,
  useConfig: () => ({
    config: useSyncExternalStore(subscribe, () => config.current),
    isLoaded: true,
    saveConfig: async (update: Partial<ThemeSettings>) => {
      if (config.holdSaves) await new Promise<void>((resolve) => (config.releaseSave = resolve))
      config.current = { ...config.current, ...update }
      notify()
    },
  }),
}))

async function renderProvider() {
  const { result } = renderHook(() => useTheme(), { wrapper: ThemeProvider })
  // Let the saved themes load.
  await act(async () => {
    await Promise.resolve()
  })
  return result
}

const cssVariable = (name: string) => document.documentElement.style.getPropertyValue(name)

beforeEach(() => {
  backend.themes = []
  config.current = { ...DEFAULT_SETTINGS }
  config.systemDark = false
  config.holdSaves = false
  localStorage.clear()
  vi.mocked(loadThemeCatalog).mockClear()
})

afterEach(cleanup)

describe("ThemeProvider", () => {
  it("chooses a library theme and caches its palette for the next start", async () => {
    const theme = await renderProvider()
    await act(async () => {
      await theme.current.loadCatalogThemes()
    })
    await act(async () => {
      await theme.current.setTheme("catalog:Dracula")
    })

    expect(config.current.theme).toBe("catalog:Dracula")
    expect(theme.current.currentTheme).toBe("catalog:Dracula")
    const dracula = theme.current.getTheme("catalog:Dracula")!
    expect(dracula.terminal.background).toBe("#282a36")
    expect(cssVariable("--background")).toBe(dracula.colors.background)
    const cache = JSON.parse(localStorage.getItem("tterm-theme-cache")!)
    expect(cache).toMatchObject({ id: "catalog:Dracula", terminal: dracula.terminal })
  })

  it("starts in a cached library theme without loading the library", async () => {
    const terminal = terminalPaletteFromScheme({
      background: "#282a36",
      foreground: "#f8f8f2",
      palette: Array.from({ length: 16 }, (_, index) => (index < 8 ? "#6272a4" : "#bd93f9")),
    })
    localStorage.setItem("tterm-cache-version", "2.0.0")
    localStorage.setItem(
      "tterm-theme-cache",
      JSON.stringify({ id: "catalog:Dracula", isCustom: true, colors: {}, terminal })
    )
    config.current = { ...DEFAULT_SETTINGS, theme: "catalog:Dracula" }
    const theme = await renderProvider()

    expect(loadThemeCatalog).not.toHaveBeenCalled()
    expect(theme.current.currentTheme).toBe("catalog:Dracula")
    // Derived afresh rather than taken from the cache.
    expect(cssVariable("--background")).toBe(deriveThemeColors(terminal).background)
  })

  it("falls back once the library turns out to lack the theme", async () => {
    config.current = { ...DEFAULT_SETTINGS, theme: "catalog:Not In This Version" }
    const theme = await renderProvider()

    await waitFor(() => expect(loadThemeCatalog).toHaveBeenCalledOnce())
    await waitFor(() => expect(document.documentElement.getAttribute("data-theme")).toBe("default"))
    expect(theme.current.currentTheme).toBe("default")
    expect(config.current.theme).toBe("catalog:Not In This Version")
  })

  it("chooses a theme created by the same handler", async () => {
    const theme = await renderProvider()
    const { createCustomTheme, setTheme } = theme.current
    await act(async () => {
      const created = await createCustomTheme({
        name: "Mine",
        colors: theme.current.getTheme("light")!.colors,
        terminal: theme.current.getTheme("light")!.terminal,
        isCustom: true,
        createdAt: 0,
        updatedAt: 0,
      } satisfies Omit<CustomTheme, "id">)
      await setTheme(created.id)
    })

    expect(config.current.theme).toMatch(/^custom-/)
    expect(theme.current.currentTheme).toBe(config.current.theme)
  })

  it("previews a theme without choosing it", async () => {
    const theme = await renderProvider()
    act(() => theme.current.previewTheme("light"))
    expect(theme.current.displayedTheme).toBe("light")
    expect(theme.current.currentTheme).toBe("default")
    expect(document.documentElement.getAttribute("data-theme")).toBe("light")

    act(() => theme.current.previewTheme(null))
    expect(theme.current.displayedTheme).toBe("default")
    expect(document.documentElement.getAttribute("data-theme")).toBe("default")
  })

  it("keeps showing a previewed theme until choosing it is saved", async () => {
    const theme = await renderProvider()
    act(() => theme.current.previewTheme("light"))
    config.holdSaves = true

    let saved: Promise<unknown> | undefined
    act(() => {
      saved = theme.current.setTheme("light")
    })
    await act(async () => {
      await Promise.resolve()
    })
    expect(theme.current.displayedTheme).toBe("light")
    expect(document.documentElement.getAttribute("data-theme")).toBe("light")

    await act(async () => {
      config.releaseSave?.()
      await saved
    })
    expect(theme.current.currentTheme).toBe("light")
    expect(theme.current.displayedTheme).toBe("light")
  })

  it("follows the system between its light and dark theme", async () => {
    config.current = { ...DEFAULT_SETTINGS, theme_follow_system: true, theme_dark: "ubuntu" }
    const theme = await renderProvider()
    expect(theme.current.currentTheme).toBe("light")
    expect(document.documentElement.getAttribute("data-theme")).toBe("light")

    act(() => {
      config.systemDark = true
      notify()
    })
    expect(theme.current.currentTheme).toBe("ubuntu")
    expect(document.documentElement.getAttribute("data-theme")).toBe("ubuntu")
  })

  it("files a chosen theme under its own appearance", async () => {
    config.current = { ...DEFAULT_SETTINGS, theme_follow_system: true, theme_dark: "ubuntu" }
    const theme = await renderProvider()

    let slot: unknown
    await act(async () => {
      slot = await theme.current.setTheme("ocean")
    })
    expect(slot).toBe("dark")
    expect(config.current).toMatchObject({ theme_dark: "ocean", theme_light: "light" })
    // The system is still light, so the light theme stays on screen.
    expect(theme.current.currentTheme).toBe("light")
    expect(document.documentElement.getAttribute("data-theme")).toBe("light")
  })

  it("keeps the theme on screen when following is switched", async () => {
    config.current = { ...DEFAULT_SETTINGS, theme: "ubuntu" }
    config.systemDark = true
    const theme = await renderProvider()

    await act(async () => {
      await theme.current.setFollowSystem(true)
    })
    expect(config.current).toMatchObject({ theme_follow_system: true, theme_dark: "ubuntu" })
    expect(theme.current.currentTheme).toBe("ubuntu")

    await act(async () => {
      await theme.current.setFollowSystem(false)
    })
    expect(config.current).toMatchObject({ theme_follow_system: false, theme: "ubuntu" })
  })

  it("keeps both of two quick stars", async () => {
    const theme = await renderProvider()
    const { toggleFavoriteTheme } = theme.current
    await act(async () => {
      await Promise.all([
        toggleFavoriteTheme("catalog:Nord"),
        toggleFavoriteTheme("catalog:Dracula"),
      ])
    })
    expect(theme.current.favoriteThemes).toEqual(["catalog:Nord", "catalog:Dracula"])

    await act(async () => {
      await theme.current.toggleFavoriteTheme("catalog:Nord")
    })
    expect(theme.current.favoriteThemes).toEqual(["catalog:Dracula"])
  })

  it("lets go of a deleted theme wherever settings name it", async () => {
    backend.themes = [{ id: "custom-1", name: "Mine", colors: {}, isCustom: true }]
    config.current = {
      theme: "custom-1",
      theme_follow_system: false,
      theme_light: "custom-1",
      theme_dark: "ubuntu",
      favorite_themes: ["custom-1", "catalog:Nord"],
    }
    const theme = await renderProvider()

    await act(async () => {
      await theme.current.deleteCustomTheme("custom-1")
    })
    expect(config.current).toEqual({
      theme: "default",
      theme_follow_system: false,
      theme_light: "light",
      theme_dark: "ubuntu",
      favorite_themes: ["catalog:Nord"],
    })
  })
})
