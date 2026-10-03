import { describe, expect, it } from "vitest"

import { parseThemeFile, ThemeImportError } from "@/lib/themeImport"

const ANSI = [
  "#000000",
  "#cd3131",
  "#0dbc79",
  "#e5e510",
  "#2472c8",
  "#bc3fbc",
  "#11a8cd",
  "#e5e5e5",
  "#666666",
  "#f14c4c",
  "#23d18b",
  "#f5f543",
  "#3b8eea",
  "#d670d6",
  "#29b8db",
  "#ffffff",
]

const EXPECTED = {
  background: "#1e1e1e",
  foreground: "#cccccc",
  palette: ANSI,
}

const NAMES = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"]

function component(hex: string, offset: number) {
  return (parseInt(hex.slice(offset, offset + 2), 16) / 255).toFixed(6)
}

function itermColor(key: string, hex: string) {
  return `<key>${key}</key>
	<dict>
		<key>Alpha Component</key>
		<real>1</real>
		<key>Blue Component</key>
		<real>${component(hex, 5)}</real>
		<key>Color Space</key>
		<string>sRGB</string>
		<key>Green Component</key>
		<real>${component(hex, 3)}</real>
		<key>Red Component</key>
		<real>${component(hex, 1)}</real>
	</dict>`
}

describe("theme import", () => {
  it("reads a Ghostty theme", () => {
    const text = [
      ...ANSI.map((color, index) => `palette = ${index}=${color}`),
      "background = #1e1e1e",
      "foreground = cccccc",
      "cursor-color = #aeafad",
      "selection-background = #264f78",
    ].join("\n")
    const result = parseThemeFile(text, "Dark Modern")
    expect(result.format).toBe("ghostty")
    expect(result.schemes).toEqual([
      {
        ...EXPECTED,
        name: "Dark Modern",
        cursor: "#aeafad",
        selectionBackground: "#264f78",
      },
    ])
  })

  it("reads a kitty theme with its name", () => {
    const text = [
      "## name: Dark Modern",
      "background #1e1e1e",
      "foreground #cccccc",
      "cursor_text_color #000000",
      ...ANSI.map((color, index) => `color${index} ${color}`),
    ].join("\n")
    const [scheme] = parseThemeFile(text, "theme.conf").schemes
    expect(scheme).toMatchObject({ ...EXPECTED, name: "Dark Modern", cursorText: "#000000" })
  })

  it("reads an Alacritty TOML theme", () => {
    const section = (name: string, colors: string[]) =>
      [`[colors.${name}]`, ...NAMES.map((key, index) => `${key} = "${colors[index]}"`)].join("\n")
    const text = [
      "[colors.primary]",
      "background = '#1e1e1e'",
      "foreground = '#cccccc'",
      section("normal", ANSI.slice(0, 8)),
      section("bright", ANSI.slice(8)),
      "[colors.cursor]",
      'text = "#1e1e1e"',
      'cursor = "#aeafad"',
    ].join("\n")
    const result = parseThemeFile(text, "dark_modern.toml")
    expect(result.format).toBe("alacritty")
    expect(result.schemes[0]).toMatchObject({
      ...EXPECTED,
      name: "dark_modern",
      cursor: "#aeafad",
      cursorText: "#1e1e1e",
    })
  })

  it("reads an older Alacritty YAML theme", () => {
    const block = (name: string, colors: string[]) => [
      `  ${name}:`,
      ...NAMES.map((key, index) => `    ${key}: '0x${colors[index].slice(1)}'`),
    ]
    const text = [
      "colors:",
      "  primary:",
      "    background: '0x1e1e1e'",
      "    foreground: '0xcccccc'",
      ...block("normal", ANSI.slice(0, 8)),
      ...block("bright", ANSI.slice(8)),
    ].join("\n")
    expect(parseThemeFile(text).schemes[0]).toMatchObject(EXPECTED)
  })

  it("reads an iTerm2 color preset", () => {
    const text = `<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
	${ANSI.map((color, index) => itermColor(`Ansi ${index} Color`, color)).join("\n\t")}
	${itermColor("Background Color (Dark)", "#1e1e1e")}
	${itermColor("Background Color (Light)", "#ffffff")}
	${itermColor("Foreground Color", "#cccccc")}
	${itermColor("Selected Text Color", "#ffffff")}
</dict>
</plist>`
    const result = parseThemeFile(text, "Dark Modern.itermcolors")
    expect(result.format).toBe("iterm")
    expect(result.schemes[0]).toMatchObject({
      ...EXPECTED,
      name: "Dark Modern",
      selectionForeground: "#ffffff",
    })
  })

  it("reads the schemes of a Windows Terminal settings file", () => {
    const scheme = (name: string) => ({
      name,
      background: "#1E1E1E",
      foreground: "#CCCCCC",
      cursorColor: "#AEAFAD",
      ...Object.fromEntries(
        NAMES.flatMap((key, index) => {
          const alias = key === "magenta" ? "purple" : key
          return [
            [alias, ANSI[index]],
            [`bright${alias[0].toUpperCase()}${alias.slice(1)}`, ANSI[index + 8]],
          ]
        })
      ),
    })
    const text = `{
      // Comments and trailing commas are allowed here.
      "profiles": { "defaults": { "font": { "face": "Cascadia // Code" } } },
      "schemes": [
        ${JSON.stringify(scheme("One"))},
        { "name": "Broken", "background": "#000000" },
        ${JSON.stringify(scheme("Two"))},
      ],
    }`
    const result = parseThemeFile(text, "settings.json")
    expect(result.format).toBe("windowsTerminal")
    expect(result.schemes.map((entry) => entry.name)).toEqual(["One", "Two"])
    expect(result.schemes[0]).toMatchObject({ ...EXPECTED, cursor: "#aeafad" })
  })

  it("names the colors a file lacks", () => {
    const error = (() => {
      try {
        parseThemeFile("palette = 0=#000000\nbackground = #111111")
      } catch (caught) {
        return caught
      }
    })()
    expect(error).toBeInstanceOf(ThemeImportError)
    expect((error as ThemeImportError).missing).toEqual([
      "foreground",
      ...Array.from({ length: 15 }, (_, index) => `color${index + 1}`),
    ])
  })

  it("rejects text it cannot place", () => {
    expect(() => parseThemeFile("hello world")).toThrow(ThemeImportError)
  })
})
