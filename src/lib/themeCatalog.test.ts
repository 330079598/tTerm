import { describe, expect, it } from "vitest"

import { colorToHex, contrastRatio, relativeLuminance } from "@/components/ThemeEditor/colorUtils"
import { catalogThemeName, isCatalogThemeId, loadThemeCatalog } from "@/lib/themeCatalog"
import { deriveThemeColors, terminalPaletteFromScheme } from "@/lib/themeDerivation"
import type { ThemeColors } from "@/types/theme"
import { THEME_COLOR_KEYS } from "@/types/theme"

const HSL_TOKEN = /^\d+(\.\d+)? \d+(\.\d+)?% \d+(\.\d+)?%$/

// Rounding to whole HSL units costs a little contrast.
const SLACK = 0.25

function contrast(colors: ThemeColors, text: keyof ThemeColors, fill: keyof ThemeColors) {
  return contrastRatio(colorToHex(colors[text])!, colorToHex(colors[fill])!)!
}

/** The most any text on the same side (lighter or darker) of `fill` can reach. */
function bestContrast(colors: ThemeColors, text: keyof ThemeColors, fill: keyof ThemeColors) {
  const fillHex = colorToHex(colors[fill])!
  const lighter = relativeLuminance(colorToHex(colors[text])!)! > relativeLuminance(fillHex)!
  return contrastRatio(lighter ? "#ffffff" : "#000000", fillHex)!
}

const dracula = {
  name: "Dracula",
  background: "#282a36",
  foreground: "#f8f8f2",
  palette: [
    "#21222c",
    "#ff5555",
    "#50fa7b",
    "#f1fa8c",
    "#bd93f9",
    "#ff79c6",
    "#8be9fd",
    "#f8f8f2",
    "#6272a4",
    "#ff6e6e",
    "#69ff94",
    "#ffffa5",
    "#d6acff",
    "#ff92df",
    "#a4ffff",
    "#ffffff",
  ],
  cursor: "#f8f8f2",
  cursorText: "#282a36",
  selectionBackground: "#44475a",
  selectionForeground: "#ffffff",
}

describe("theme derivation", () => {
  it("maps a scheme onto the terminal palette", () => {
    const palette = terminalPaletteFromScheme(dracula)
    expect(palette).toMatchObject({
      background: "#282a36",
      foreground: "#f8f8f2",
      cursor: "#f8f8f2",
      cursorAccent: "#282a36",
      selectionBackground: "#44475a",
      selectionForeground: "#ffffff",
      black: "#21222c",
      blue: "#bd93f9",
      brightWhite: "#ffffff",
    })
  })

  it("fills in the colors a scheme leaves out", () => {
    const palette = terminalPaletteFromScheme({
      background: "#000",
      foreground: "#fff",
      palette: dracula.palette,
    })
    expect(palette.cursor).toBe("#ffffff")
    expect(palette.selectionBackground).toBe("rgba(255, 255, 255, 0.25)")
    expect(palette.selectionForeground).toBeUndefined()
  })

  it("builds the interface from the palette's own colors", () => {
    const colors = deriveThemeColors(terminalPaletteFromScheme(dracula))
    expect(colorToHex(colors.background)).toBe("#282a36")
    expect(colorToHex(colors.tabBackground)).toBe("#282a36")
    // Dracula's blue is its purple.
    expect(colors.primary).toBe(colors.ring)
    expect(colorToHex(colors.primary)).toBe("#bd93f9")
    expect(colorToHex(colors.destructive)).toBe("#ff5555")
  })

  it("falls back to another hue when the blue is gray", () => {
    const palette = terminalPaletteFromScheme({
      ...dracula,
      palette: dracula.palette.map((color, index) =>
        index === 4 || index === 12 ? "#808080" : color
      ),
      cursor: "#ff9900",
    })
    expect(colorToHex(deriveThemeColors(palette).primary)).toBe("#ff9900")
  })
})

describe("theme catalog", () => {
  it("loads every library theme under a unique id", async () => {
    const themes = await loadThemeCatalog()
    expect(themes.length).toBeGreaterThan(600)
    expect(new Set(themes.map((theme) => theme.id)).size).toBe(themes.length)
    const theme = themes.find((candidate) => candidate.name === "Dracula")!
    expect(isCatalogThemeId(theme.id)).toBe(true)
    expect(catalogThemeName(theme.id)).toBe("Dracula")
    expect(theme.isDark).toBe(true)
    expect(themes.find((candidate) => candidate.name === "Atom One Light")?.isDark).toBe(false)
  })

  it("derives a library theme's interface colors once, on first use", async () => {
    const [theme] = await loadThemeCatalog()
    expect(Object.getOwnPropertyDescriptor(theme, "colors")?.get).toBeTypeOf("function")
    expect(theme.colors).toBe(theme.colors)
    expect(theme.colors).toEqual(deriveThemeColors(theme.terminal))
  })

  it("derives readable interface colors for every library theme", async () => {
    for (const theme of await loadThemeCatalog()) {
      const { colors } = theme
      for (const key of THEME_COLOR_KEYS) {
        expect(colors[key], `${theme.name} ${key}`).toMatch(HSL_TOKEN)
      }
      const readable: Array<[keyof ThemeColors, keyof ThemeColors, number]> = [
        ["foreground", "background", 4.5],
        ["foreground", "card", 4.5],
        ["foreground", "muted", 4.5],
        ["mutedForeground", "card", 4.5],
        ["foreground", "accent", 4.5],
        ["mutedForeground", "background", 4.5],
        ["successForeground", "background", 4.5],
        ["warningForeground", "background", 4.5],
        ["primaryForeground", "primary", 4.5],
        ["destructiveForeground", "destructive", 3],
        ["destructive", "background", 3],
      ]
      for (const [text, fill, ratio] of readable) {
        const required = Math.min(ratio, bestContrast(colors, text, fill))
        expect(contrast(colors, text, fill), `${theme.name}: ${text} on ${fill}`).toBeGreaterThan(
          required - SLACK
        )
      }
    }
  })
})
