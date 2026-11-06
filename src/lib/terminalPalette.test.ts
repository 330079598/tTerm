import { describe, expect, it } from "vitest"

import { colorToHex } from "@/components/ThemeEditor/colorUtils"
import { PRESET_THEMES } from "@/lib/themeDefinitions"
import { generateTerminalPaletteFromColors } from "@/lib/terminalPalette"

/**
 * These values are also written down in the Rust tests in
 * `crates/tterm-terminal/src/palette.rs`. Changing the default theme or the
 * derivation has to update both sides.
 */
describe("terminal palette defaults", () => {
  it("keeps the default theme's explicit palette", () => {
    const palette = PRESET_THEMES[0].terminal
    expect({
      background: colorToHex(palette.background),
      foreground: colorToHex(palette.foreground),
      cursor: colorToHex(palette.cursor),
      selectionBackground: palette.selectionBackground,
      black: colorToHex(palette.black),
      red: colorToHex(palette.red),
      green: colorToHex(palette.green),
      yellow: colorToHex(palette.yellow),
      blue: colorToHex(palette.blue),
      magenta: colorToHex(palette.magenta),
      cyan: colorToHex(palette.cyan),
      white: colorToHex(palette.white),
      brightBlack: colorToHex(palette.brightBlack),
      brightRed: colorToHex(palette.brightRed),
      brightGreen: colorToHex(palette.brightGreen),
      brightYellow: colorToHex(palette.brightYellow),
      brightBlue: colorToHex(palette.brightBlue),
      brightMagenta: colorToHex(palette.brightMagenta),
      brightCyan: colorToHex(palette.brightCyan),
      brightWhite: colorToHex(palette.brightWhite),
    }).toEqual({
      background: "#1b1d23",
      foreground: "#d7dde8",
      cursor: "#79d8ff",
      selectionBackground: "rgba(77, 182, 255, 0.22)",
      black: "#161b22",
      red: "#f16c75",
      green: "#7bd88f",
      yellow: "#e6c16a",
      blue: "#6aaeff",
      magenta: "#c792ea",
      cyan: "#66d9ef",
      white: "#c8d2dc",
      brightBlack: "#5c6773",
      brightRed: "#ff8b94",
      brightGreen: "#9ee6b8",
      brightYellow: "#f2d38f",
      brightBlue: "#8fc7ff",
      brightMagenta: "#ddb7ff",
      brightCyan: "#8be9fd",
      brightWhite: "#f4f7fb",
    })
  })

  it("derives the same palette as the Rust side", () => {
    const palette = generateTerminalPaletteFromColors(PRESET_THEMES[0].colors)
    expect(palette.selectionBackground).toBe("hsl(207 100% 40% / 0.24)")
    expect(palette.cursorAccent).toBeUndefined()
    expect(palette.selectionForeground).toBeUndefined()
    expect(
      Object.fromEntries(Object.entries(palette).map(([key, value]) => [key, colorToHex(value)]))
    ).toEqual({
      background: "#1b1d23",
      foreground: "#cccccc",
      cursor: "#005aa3",
      selectionBackground: "#0070cc",
      black: "#16181d",
      red: "#ef4343",
      green: "#1eb857",
      yellow: "#f5a314",
      blue: "#0070cc",
      magenta: "#5c07ed",
      cyan: "#0070cc",
      white: "#bdbdbd",
      brightBlack: "#1d2025",
      brightRed: "#eb1e1e",
      brightGreen: "#1a9e4a",
      brightYellow: "#d78c09",
      brightBlue: "#005aa3",
      brightMagenta: "#4c06c6",
      brightCyan: "#005aa3",
      brightWhite: "#4f4f4f",
    })
  })
})
