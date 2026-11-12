// Runs the real TypeScript theme modules and prints the values the Rust port
// is tested against. It loads src/ without changing it, the way
// keyword-highlight-oracle.mjs does.
import fs from "node:fs"
import path from "node:path"
import ts from "typescript"

const ROOT = path.resolve(import.meta.dirname, "../..")

function read(relative) {
  return fs.readFileSync(path.join(ROOT, relative), "utf8")
}

function dataModule(source) {
  return `data:text/javascript;base64,${Buffer.from(source).toString("base64")}`
}

function moduleUrl(relative, imports = {}) {
  let source = ts.transpileModule(read(relative), {
    compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ESNext },
  }).outputText
  for (const [specifier, url] of Object.entries(imports)) {
    source = source.replaceAll(`"${specifier}"`, JSON.stringify(url))
  }
  return dataModule(source)
}

function fnv1a64(value) {
  let hash = 0xcbf29ce484222325n
  for (const byte of Buffer.from(value, "utf8")) {
    hash ^= BigInt(byte)
    hash = (hash * 0x100000001b3n) & 0xffffffffffffffffn
  }
  return hash.toString(16).padStart(16, "0")
}

const typesUrl = moduleUrl("src/types/theme.ts")
const colorUrl = moduleUrl("src/components/ThemeEditor/colorUtils.ts", {
  "@/types/theme": typesUrl,
})
const definitionsUrl = moduleUrl("src/lib/themeDefinitions.ts", { "@/types/theme": typesUrl })
const derivationUrl = moduleUrl("src/lib/themeDerivation.ts", {
  "@/components/ThemeEditor/colorUtils": colorUrl,
  "@/types/theme": typesUrl,
})
const terminalPaletteUrl = moduleUrl("src/lib/terminalPalette.ts", {
  "@/components/ThemeEditor/colorUtils": colorUrl,
  "@/types/theme": typesUrl,
})
const importUrl = moduleUrl("src/lib/themeImport.ts", {
  "@/components/ThemeEditor/colorUtils": colorUrl,
  "@/lib/themeDerivation": derivationUrl,
})
const catalogUrl = moduleUrl("src/lib/themeCatalog.ts", {
  "@/lib/themeDerivation": derivationUrl,
  "@/types/theme": typesUrl,
  "@/assets/themes/catalog.json": dataModule(
    `export default ${read("src/assets/themes/catalog.json")}`
  ),
})
const themeUtilsUrl = moduleUrl("src/lib/themeUtils.ts", {
  "@/lib/themeDefinitions": definitionsUrl,
  "@/types/theme": typesUrl,
})
const customThemesUrl = moduleUrl("src/lib/customThemes.ts", {
  "@/lib/themeDefinitions": definitionsUrl,
  "@/types/theme": typesUrl,
})

const types = await import(typesUrl)
const colors = await import(colorUrl)
const definitions = await import(definitionsUrl)
const derivation = await import(derivationUrl)
const terminalPalette = await import(terminalPaletteUrl)
const themeImport = await import(importUrl)
const catalog = await import(catalogUrl)
const themeUtils = await import(themeUtilsUrl)
const customThemes = await import(customThemesUrl)

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

function importCase(name, text, fileName = "") {
  try {
    return { name, text, fileName, ok: true, value: themeImport.parseThemeFile(text, fileName) }
  } catch (error) {
    return {
      name,
      text,
      fileName,
      ok: false,
      error:
        error instanceof themeImport.ThemeImportError
          ? {
              name: error.name,
              reason: error.reason,
              missing: error.missing,
              message: error.message,
            }
          : { name: error?.name ?? "Error", message: String(error?.message ?? error) },
    }
  }
}

function paletteCase(name, scheme) {
  try {
    return {
      name,
      scheme,
      ok: true,
      palette: derivation.terminalPaletteFromScheme(scheme),
      colors: derivation.deriveThemeColors(derivation.terminalPaletteFromScheme(scheme)),
      dark: derivation.isDarkPalette(derivation.terminalPaletteFromScheme(scheme)),
    }
  } catch (error) {
    return { name, scheme, ok: false, error: String(error?.message ?? error) }
  }
}

function generatedCase(name, value) {
  try {
    return {
      name,
      colors: value,
      ok: true,
      palette: terminalPalette.generateTerminalPaletteFromColors(value),
    }
  } catch (error) {
    return { name, colors: value, ok: false, error: String(error?.message ?? error) }
  }
}

function normalizeCase(name, themes) {
  return { name, themes, value: customThemes.normalizeCustomThemes(themes) }
}

const itermComponent = (hex, offset) =>
  (parseInt(hex.slice(offset, offset + 2), 16) / 255).toFixed(6)

function itermColor(key, hex) {
  return `<key>${key}</key>
\t<dict>
\t\t<key>Alpha Component</key>
\t\t<real>1</real>
\t\t<key>Blue Component</key>
\t\t<real>${itermComponent(hex, 5)}</real>
\t\t<key>Color Space</key>
\t\t<string>sRGB</string>
\t\t<key>Green Component</key>
\t\t<real>${itermComponent(hex, 3)}</real>
\t\t<key>Red Component</key>
\t\t<real>${itermComponent(hex, 1)}</real>
\t</dict>`
}

function itermBody(entries) {
  return `<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
${entries.map(([key, hex]) => itermColor(key, hex)).join("\n")}
</dict>
</plist>`
}

const names = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"]
const ghosttyText = (extra = {}) =>
  [
    ...ANSI.map((color, index) => `palette = ${index}=${color}`),
    `background = ${extra.background ?? "#1e1e1e"}`,
    `foreground = ${extra.foreground ?? "#cccccc"}`,
    ...(extra.lines ?? []),
  ].join(extra.newline ?? "\n")

const alacrittySection = (name, values) =>
  [`[colors.${name}]`, ...names.map((key, index) => `${key} = "${values[index]}"`)].join("\n")

const windowsScheme = (schemeName, overrides = {}) => ({
  name: schemeName,
  background: "#1E1E1E",
  foreground: "#CCCCCC",
  cursorColor: "#AEAFAD",
  ...Object.fromEntries(
    names.flatMap((key, index) => {
      const alias = key === "magenta" ? "purple" : key
      return [
        [alias, ANSI[index]],
        [`bright${alias[0].toUpperCase()}${alias.slice(1)}`, ANSI[index + 8]],
      ]
    })
  ),
  ...overrides,
})

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

const importCases = [
  importCase(
    "ghostty from the TypeScript test",
    ghosttyText({
      lines: ["cursor-color = #aeafad", "selection-background = #264f78"],
    }),
    "Dark Modern"
  ),
  importCase("ghostty missing colors", "palette = 0=#000000\nbackground = #111111"),
  importCase(
    "ghostty invalid colors",
    ghosttyText().replace("palette = 0=#000000", "palette = 0=nonsense"),
    "bad.txt"
  ),
  importCase(
    "ghostty ignores extra keys and indexes past fifteen",
    `${ghosttyText({
      lines: ["palette = 16=#abcdef", "palette = 999=#abcdef", "font-family = Fira Code"],
    })}\nwindow-padding = 4`,
    "extra.conf"
  ),
  importCase(
    "ghostty last duplicate palette entry wins",
    `${ghosttyText()}\npalette = 3=#123456`,
    "duplicate.conf"
  ),
  importCase(
    "ghostty CRLF and surrounding JavaScript whitespace",
    `\uFEFF\u3000${ghosttyText({ newline: "\r\n" })}\u00A0`,
    "crlf.conf"
  ),
  importCase(
    "kitty from the TypeScript test",
    [
      "## name: Dark Modern",
      "background #1e1e1e",
      "foreground #cccccc",
      "cursor_text_color #000000",
      ...ANSI.map((color, index) => `color${index} ${color}`),
    ].join("\n"),
    "theme.conf"
  ),
  importCase(
    "kitty without a name uses the file name",
    [
      "background #1e1e1e",
      "foreground #cccccc",
      ...ANSI.map((color, index) => `color${index} ${color}`),
    ].join("\n"),
    "  My Theme.conf  "
  ),
  importCase(
    "kitty ignores color sixteen and special whitespace",
    [
      "## name:   Spaced Name   ",
      "background   #1e1e1e",
      "foreground\t#cccccc",
      "color16 #ffffff",
      ...ANSI.map((color, index) => `color${index} ${color}`),
    ].join("\n"),
    "spaced.conf"
  ),
  importCase(
    "alacritty TOML from the TypeScript test",
    [
      "[colors.primary]",
      "background = '#1e1e1e'",
      "foreground = '#cccccc'",
      alacrittySection("normal", ANSI.slice(0, 8)),
      alacrittySection("bright", ANSI.slice(8)),
      "[colors.cursor]",
      'text = "#1e1e1e"',
      'cursor = "#aeafad"',
    ].join("\n"),
    "dark_modern.toml"
  ),
  importCase(
    "alacritty YAML from the TypeScript test",
    [
      "colors:",
      "  primary:",
      "    background: '0x1e1e1e'",
      "    foreground: '0xcccccc'",
      ...["normal", "bright"].flatMap((section, bright) => [
        `  ${section}:`,
        ...names.map(
          (key, index) => `    ${key}: '0x${ANSI[index + bright * 8].slice(1)}'`
        ),
      ]),
    ].join("\n"),
    "old.yml"
  ),
  importCase(
    "alacritty cursor and selection sections",
    [
      "[colors.primary]",
      "background = '#1e1e1e'",
      "foreground = '#cccccc'",
      alacrittySection("normal", ANSI.slice(0, 8)),
      alacrittySection("bright", ANSI.slice(8)),
      "[colors.cursor]",
      'cursor = "#aeafad"',
      'text = "#1e1e1e"',
      "[colors.selection]",
      'background = "#264f78"',
      'text = "#ffffff"',
    ].join("\n"),
    "selection.toml"
  ),
  importCase(
    "alacritty missing one bright color",
    [
      "[colors.primary]",
      "background = '#1e1e1e'",
      "foreground = '#cccccc'",
      alacrittySection("normal", ANSI.slice(0, 8)),
      alacrittySection("bright", ANSI.slice(8)).replace('brightWhite = "#ffffff"', ""),
    ].join("\n"),
    "missing.toml"
  ),
  importCase(
    "iterm from the TypeScript test",
    itermBody([
      ...ANSI.map((color, index) => [`Ansi ${index} Color`, color]),
      ["Background Color (Dark)", "#1e1e1e"],
      ["Background Color (Light)", "#ffffff"],
      ["Foreground Color", "#cccccc"],
      ["Selected Text Color", "#ffffff"],
    ]),
    "Dark Modern.itermcolors"
  ),
  importCase(
    "iterm uses the dark appearance",
    itermBody([
      ...ANSI.map((color, index) => [`Ansi ${index} Color`, color]),
      ["Background Color (Dark)", "#1e1e1e"],
      ["Background Color (Light)", "#ffffff"],
      ["Foreground Color", "#cccccc"],
    ]),
    "dark.itermcolors"
  ),
  importCase(
    "iterm skips colors with a missing component",
    itermBody([...ANSI.map((color, index) => [`Ansi ${index} Color`, color])]).replace(
      "<key>Blue Component</key>\n\t\t<real>0.000000</real>",
      ""
    ),
    "broken.itermcolors"
  ),
  importCase(
    "windows terminal settings from the TypeScript test",
    `{
      // Comments and trailing commas are allowed here.
      "profiles": { "defaults": { "font": { "face": "Cascadia // Code" } } },
      "schemes": [
        ${JSON.stringify(windowsScheme("One"))},
        { "name": "Broken", "background": "#000000" },
        ${JSON.stringify(windowsScheme("Two"))},
      ],
    }`,
    "settings.json"
  ),
  importCase(
    "windows terminal single scheme",
    JSON.stringify(windowsScheme("Single")),
    "single.json"
  ),
  importCase(
    "windows terminal array with a leading broken entry",
    JSON.stringify([{ name: "Broken" }, windowsScheme("Good")]),
    "array.json"
  ),
  importCase("windows terminal empty array", "[]", "empty.json"),
  importCase(
    "windows terminal invalid JSON",
    `{
      "schemes": [${JSON.stringify(windowsScheme("One"))}],
    `,
    "broken.json"
  ),
  importCase(
    "windows terminal comments inside strings survive",
    `{
      "schemes": [{
        "name": "http://example.test/*not a comment*/",
        "background": "#1e1e1e",
        "foreground": "#cccccc",
        ${names
          .flatMap((key, index) => {
            const alias = key === "magenta" ? "purple" : key
            return [
              `"${alias}": "${ANSI[index]}"`,
              `"bright${alias[0].toUpperCase()}${alias.slice(1)}": "${ANSI[index + 8]}"`,
            ]
          })
          .join(",\n        ")}
      }],
    }`,
    "comments.json"
  ),
  importCase("unknown format", "hello world", "notes.txt"),
  importCase("empty text", "   \u00A0 ", "empty.txt"),
  importCase("null JSON is not a theme", "null", "null.json"),
  importCase("file name fallback keeps the last extension", ghosttyText(), ".itermcolors"),
  importCase(
    "file name fallback strips only the last extension",
    ghosttyText(),
    " my.theme.name.json "
  ),
  importCase(
    "file name fallback keeps emoji and combining marks",
    ghosttyText(),
    " 图标😀e\u0301.json "
  ),
  importCase("file name fallback handles a trailing newline", ghosttyText(), "newline.json\n"),
  importCase("file name fallback when the name is empty", ghosttyText(), ""),
  importCase("file name fallback when only a dot is left", ghosttyText(), "."),
]

const paletteCases = [
  paletteCase("Dracula from the TypeScript test", dracula),
  paletteCase("scheme without optional colors", {
    background: "#000",
    foreground: "#fff",
    palette: dracula.palette,
  }),
  paletteCase("invalid optional colors", {
    name: "Broken",
    background: "#123456",
    foreground: "bogus",
    palette: ["bogus", ...Array.from({ length: 15 }, () => "")],
    cursor: "bogus",
    cursorText: "bogus",
    selectionBackground: "bogus",
    selectionForeground: "bogus",
  }),
  paletteCase("empty palette", { background: "#000", foreground: "#fff", palette: [] }),
  paletteCase("palette with holes", {
    background: "#000",
    foreground: "#fff",
    palette: [undefined, "#ffffff"],
  }),
  paletteCase("dark and light detection", {
    background: "hsl(0 0% 100%)",
    foreground: "#000000",
    palette: dracula.palette,
  }),
  paletteCase("invalid background and foreground", {
    background: "invalid",
    foreground: "",
    palette: dracula.palette,
  }),
]

const generatedCases = [
  generatedCase("default preset colors", definitions.PRESET_THEMES[0].colors),
  generatedCase("light preset colors", definitions.PRESET_THEMES[1].colors),
  generatedCase("ocean preset colors", definitions.PRESET_THEMES[2].colors),
  generatedCase("forest preset colors", definitions.PRESET_THEMES[3].colors),
  generatedCase("sunset preset colors", definitions.PRESET_THEMES[4].colors),
  generatedCase("ubuntu preset colors", definitions.PRESET_THEMES[5].colors),
  generatedCase("fractional hsl values", {
    background: "220.5 13.5% 12.25%",
    foreground: "0.5 0% 80.75%",
    primary: "207.25 100% 40.5%",
    accent: "206.75 99.5% 41%",
    success: "142.5 72.25% 42.5%",
    warning: "38.25 91.75% 52.5%",
    destructive: "0.75 84.25% 60.5%",
    muted: "220 13% 23%",
    mutedForeground: "0 0% 59%",
  }),
  generatedCase("invalid hsl values fall back", {
    background: "bogus",
    foreground: "",
    primary: "",
    accent: "",
    success: "",
    warning: "",
    destructive: "",
    muted: "",
    mutedForeground: "",
  }),
  generatedCase("tiny hsl values keep JavaScript number formatting", {
    background: "0.0000001 0% 0%",
    foreground: "0.000001 0% 0%",
    primary: "0 0% 0%",
    accent: "0 0% 0%",
    success: "0 0% 0%",
    warning: "0 0% 0%",
    destructive: "0 0% 0%",
    muted: "0 0% 0%",
    mutedForeground: "0 0% 0%",
  }),
]

const parseHslCases = [
  "220 13% 12%",
  "hsl(220 13% 12%)",
  "  1 2% 3%  ",
  "1 2 3",
  "1% 2 3",
  "abc",
  "1.5 2.5% 3.5%",
  "999999999999999999999 1% 2%",
  "12. .5% 3%",
  "\u30001\u00A02%\uFEFF3%",
]

const derivedColors = ["default", "light", "ocean", "forest", "sunset", "ubuntu"].map((id) => {
  const theme = definitions.PRESET_THEMES.find((candidate) => candidate.id === id)
  return {
    name: id,
    palette: theme.terminal,
    colors: derivation.deriveThemeColors(theme.terminal),
  }
})
derivedColors.push({
  name: "Dracula",
  palette: derivation.terminalPaletteFromScheme(dracula),
  colors: derivation.deriveThemeColors(derivation.terminalPaletteFromScheme(dracula)),
})
const grayBlue = derivation.terminalPaletteFromScheme({
  ...dracula,
  palette: dracula.palette.map((color, index) => (index === 4 || index === 12 ? "#808080" : color)),
  cursor: "#ff9900",
})
derivedColors.push({
  name: "gray blue fallback",
  palette: grayBlue,
  colors: derivation.deriveThemeColors(grayBlue),
})
const invalidPalette = Object.fromEntries(
  [
    "background",
    "foreground",
    "cursor",
    "selectionBackground",
    "black",
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
    "white",
    "brightBlack",
    "brightRed",
    "brightGreen",
    "brightYellow",
    "brightBlue",
    "brightMagenta",
    "brightCyan",
    "brightWhite",
  ].map((key) => [key, key === "foreground" ? "" : "bogus"])
)
derivedColors.push({
  name: "invalid palette colors",
  palette: invalidPalette,
  colors: derivation.deriveThemeColors(invalidPalette),
})

const catalogThemes = await catalog.loadThemeCatalog()
const catalogDigest = fnv1a64(
  catalogThemes
    .map((theme) =>
      [
        theme.id,
        theme.name,
        String(theme.isDark),
        ...types.THEME_COLOR_KEYS.map((key) => theme.colors[key]),
        ...Object.values(theme.terminal),
      ].join("\u0000")
    )
    .join("\u0001")
)
const nonHslColors = catalogThemes.flatMap((theme) =>
  types.THEME_COLOR_KEYS.filter(
    (key) => !/^\d+(\.\d+)? \d+(\.\d+)?% \d+(\.\d+)?%$/.test(theme.colors[key])
  ).map((key) => `${theme.name} ${key}`)
)
const deriveMismatches = catalogThemes.filter(
  (theme) =>
    JSON.stringify(theme.colors) !== JSON.stringify(derivation.deriveThemeColors(theme.terminal))
)
const contrastSlack = 0.25
function contrast(colorsValue, text, fill) {
  return colors.contrastRatio(
    colors.colorToHex(colorsValue[text]),
    colors.colorToHex(colorsValue[fill])
  )
}
function bestContrast(colorsValue, text, fill) {
  const fillHex = colors.colorToHex(colorsValue[fill])
  const lighter =
    colors.relativeLuminance(colors.colorToHex(colorsValue[text])) >
    colors.relativeLuminance(fillHex)
  return colors.contrastRatio(lighter ? "#ffffff" : "#000000", fillHex)
}
const readable = [
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
const contrastFailures = catalogThemes.flatMap((theme) =>
  readable
    .filter(([text, fill, ratio]) => {
      const required = Math.min(ratio, bestContrast(theme.colors, text, fill))
      return !(contrast(theme.colors, text, fill) > required - contrastSlack)
    })
    .map(([text, fill]) => `${theme.name}: ${text} on ${fill}`)
)

const now = 1700000000000
const realNow = Date.now
Date.now = () => now

const preset = definitions.PRESET_THEMES[2]
const customOverride = {
  id: "ocean",
  name: "Ocean Override",
  description: "changed",
  colors: { ...preset.colors, background: "200 30% 20%" },
  terminal: { ...preset.terminal, background: "#102a33" },
  baseTheme: "ocean",
  isCustom: true,
  createdAt: 1,
  updatedAt: 2,
}

const newTheme = themeUtils.createCustomThemeFromPreset("ocean", "Mine", undefined)
const newThemeWithDescription = themeUtils.createCustomThemeFromPreset(
  "sunset",
  "Warm",
  "A description"
)
const newThemeUnknownPreset = themeUtils.createCustomThemeFromPreset("nope", "Unknown")

const normalizeCases = [
  normalizeCase("mixed valid and invalid", [
    null,
    "text",
    3,
    [],
    {},
    { id: "", name: "No id", colors: {} },
    { id: "a", name: "", colors: {} },
    { id: "b", name: "B" },
    { id: "c", name: "C", colors: {} },
    {
      id: "d",
      name: "D",
      description: null,
      baseTheme: "light",
      colors: {
        ...definitions.PRESET_THEMES[1].colors,
        background: "0 0% 50%",
        unknownColor: "kept",
      },
      terminal: {
        ...definitions.PRESET_THEMES[1].terminal,
        unknownTerminal: "kept",
      },
      createdAt: 0,
      updatedAt: null,
      extra: "dropped",
    },
    {
      id: "e",
      name: "E",
      colors: {},
      baseTheme: "unknown",
      terminal: definitions.PRESET_THEMES[0].terminal,
    },
  ]),
  normalizeCase("edge values", [
    { id: "array", name: "Array", colors: [], terminal: "ab" },
    { id: "empty-terminal", name: "Empty terminal", colors: {}, terminal: {} },
    { id: "null-terminal", name: "Null terminal", colors: {}, terminal: null },
    {
      id: "null-members",
      name: "Null members",
      description: null,
      colors: { background: null, unknownColor: null },
      terminal: {
        background: null,
        cursorAccent: null,
        selectionForeground: null,
        unknownTerminal: null,
      },
      baseTheme: null,
      createdAt: 0,
      updatedAt: null,
    },
  ]),
  normalizeCase("string spreads", [
    {
      id: "string-colors",
      name: "String colors",
      colors: "中文e\u0301",
      terminal: "ab",
      createdAt: 1,
      updatedAt: 2,
    },
  ]),
]

const importedTheme = {
  name: "Dracula",
  description: "Imported from iTerm2",
  colors: derivation.deriveThemeColors(derivation.terminalPaletteFromScheme(dracula)),
  terminal: derivation.terminalPaletteFromScheme(dracula),
  isCustom: true,
  createdAt: now,
  updatedAt: now,
}

const customThemeJson = JSON.stringify({ ...newTheme, id: "custom-1" })
const importedThemeJson = JSON.stringify({ ...importedTheme, id: "custom-2" })
const presetThemeJson = JSON.stringify(definitions.PRESET_THEMES[0])
const presetsJson = JSON.stringify(definitions.PRESET_THEMES)

Date.now = realNow

const output = {
  types: {
    themeColorKeys: types.THEME_COLOR_KEYS,
    presetThemeIds: types.PRESET_THEME_IDS,
  },
  definitions: {
    presets: definitions.PRESET_THEMES,
    presetsJson,
    presetThemeJson,
    getPresetTheme: ["default", "ocean", "ubuntu", "unknown", "", "constructor"].map((id) => ({
      id,
      value: definitions.getPresetTheme(id) ?? null,
    })),
    resolve: [
      { id: "default", custom: [], value: definitions.resolveThemeDefinition("default", []) },
      {
        id: "ocean",
        custom: [customOverride],
        value: definitions.resolveThemeDefinition("ocean", [customOverride]),
      },
      {
        id: "custom-x",
        custom: [customOverride],
        value: definitions.resolveThemeDefinition("custom-x", [customOverride]),
      },
      { id: "unknown", custom: [], value: definitions.resolveThemeDefinition("unknown", []) },
    ],
  },
  colors: {
    colorToHex: [
      "#abc",
      "#ABCDEF",
      "abcdef",
      "0xabcdef",
      "#abcd",
      "#abcdef00",
      "#abc\uFEFF",
      "#abc\u0085",
      " rgb(1, 2, 3) ",
      "rgba(1 2 3 0.5)",
      "RGB(1,2,3)",
      "rgb(-1,2,3)",
      "rgb(1.5,2.5,3.5)",
      "rgb(1,2,3",
      "hsl(0 100% 50%)",
      "hsl(0,100%,50%)",
      "hsla(240 100% 50% 0.5)",
      "hsl(240deg 100% 50%)",
      "0 100% 50%",
      "0 100 50%",
      "\u3000#abc\uFEFF",
      "\u00A0rgb(10\u00A020\u300030)\u00A0",
      "not a color",
      "",
    ].map((value) => ({ value, hex: colors.colorToHex(value) })),
    hexToRgb: ["#abc", "abc", "#abcdef", "abcdef", "#abcd", " #ABCDEF "].map((value) => ({
      value,
      rgb: colors.hexToRgb(value),
    })),
    rgbToHex: [
      [0, 0, 0],
      [255, 255, 255],
      [-10, 300, 127.5],
      [1.4, 2.5, 3.6],
      [0.5, 1.5, 254.5],
    ].map(([r, g, b]) => ({ input: [r, g, b], hex: colors.rgbToHex(r, g, b) })),
    hslToRgb: [
      [0, 100, 50],
      [120, 100, 25],
      [240, 0, 50],
      [220.5, 13.5, 12.25],
      [-30, 120, -5],
      [359.9, 99.9, 99.9],
    ].map(([h, s, l]) => {
      const { r, g, b } = colors.hslToRgb(h, s, l)
      return { input: [h, s, l], hex: colors.rgbToHex(r, g, b) }
    }),
    relativeLuminance: ["#000000", "#ffffff", "#282a36", "bogus"].map((value) => ({
      value,
      luminance: colors.relativeLuminance(value),
    })),
    contrastRatio: [
      ["#000000", "#ffffff"],
      ["#282a36", "#f8f8f2"],
      ["#000000", "bogus"],
    ].map(([first, second]) => ({ first, second, ratio: colors.contrastRatio(first, second) })),
  },
  terminalPalette: {
    defaultPalette: definitions.PRESET_THEMES[0].terminal,
    defaultPaletteHex: Object.fromEntries(
      Object.entries(definitions.PRESET_THEMES[0].terminal).map(([key, value]) => [
        key,
        key === "selectionBackground" ? value : colors.colorToHex(value),
      ])
    ),
    generated: generatedCases,
    parseHsl: parseHslCases.map((value) => ({ value, result: terminalPalette.parseHsl(value) })),
    withWindowBlur: [
      {
        blur: false,
        input: { background: "hsl(220 13% 12%)" },
        palette: terminalPalette.withWindowBlur({ background: "hsl(220 13% 12%)" }, false),
      },
      {
        blur: true,
        input: { background: "hsl(220 13% 12%)" },
        palette: terminalPalette.withWindowBlur({ background: "hsl(220 13% 12%)" }, true),
      },
      {
        blur: true,
        input: { background: "bogus" },
        palette: terminalPalette.withWindowBlur({ background: "bogus" }, true),
      },
      { blur: true, input: {}, palette: terminalPalette.withWindowBlur({}, true) },
      { blur: false, input: {}, palette: terminalPalette.withWindowBlur({}, false) },
    ],
    isTransparent: [
      { background: "#1b1d2300" },
      { background: "#1B1D2300" },
      { background: "#1b1d23" },
      { background: "rgba(0, 0, 0, 0)" },
      {},
    ].map((palette) => ({
      palette,
      value: terminalPalette.isTransparentTerminalTheme(palette),
    })),
  },
  derivation: {
    schemes: paletteCases,
    colors: derivedColors,
  },
  import: { cases: importCases },
  custom: {
    isPresetThemeId: [
      "default",
      "light",
      "ocean",
      "forest",
      "sunset",
      "ubuntu",
      "catalog:x",
      "",
      "Default",
      "custom-1",
    ].map((id) => ({ id, value: customThemes.isPresetThemeId(id) })),
    merge: [
      {
        name: "without override",
        value: customThemes.mergePresetThemeWithOverride(preset, undefined),
      },
      {
        name: "with override",
        value: customThemes.mergePresetThemeWithOverride(preset, customOverride),
      },
    ],
    normalize: normalizeCases,
  },
  utils: {
    now,
    createCustomThemeFromPreset: [
      { name: "ocean without description", value: { ...newTheme, id: "custom-1" } },
      { name: "sunset with description", value: { ...newThemeWithDescription, id: "custom-2" } },
      { name: "unknown preset", value: { ...newThemeUnknownPreset, id: "custom-3" } },
    ],
  },
  catalog: {
    count: catalogThemes.length,
    uniqueIds: new Set(catalogThemes.map((theme) => theme.id)).size,
    digest: catalogDigest,
    dracula: catalogThemes.find((theme) => theme.name === "Dracula"),
    atomOneLight: catalogThemes.find((theme) => theme.name === "Atom One Light"),
    first: catalogThemes[0],
    getterIdentity: catalogThemes[0].colors === catalogThemes[0].colors,
    deriveMismatchCount: deriveMismatches.length,
    nonHslColorCount: nonHslColors.length,
    contrastFailureCount: contrastFailures.length,
    isCatalogThemeId: ["catalog:Dracula", "Dracula", "", "catalog:"].map((id) => ({
      id,
      value: catalog.isCatalogThemeId(id),
    })),
    catalogThemeName: [
      "catalog:Dracula",
      "catalog:",
      "Dracula",
      "catalog:😀",
      "abcdefghij",
      "abcdefgh",
      "abcdefgh😀",
      "abc中文defghijkl",
    ].map((id) => ({
      id,
      value: catalog.catalogThemeName(id),
    })),
    isCatalogTheme: [
      { name: "catalog", value: catalog.isCatalogTheme(catalogThemes[0]) },
      { name: "preset", value: catalog.isCatalogTheme(definitions.PRESET_THEMES[0]) },
      { name: "custom", value: catalog.isCatalogTheme(customOverride) },
      { name: "undefined", value: catalog.isCatalogTheme(undefined) },
    ],
    createdFromPalette: catalog.createCatalogTheme("catalog:Fixed", {
      ...definitions.PRESET_THEMES[0].terminal,
    }),
    createdFromScheme: catalog.catalogThemeFromScheme(dracula),
  },
  serialization: {
    presetThemeJson,
    presetsJson,
    customThemeJson,
    importedThemeJson,
    normalizedMixedJson: JSON.stringify(normalizeCases[0].value),
    normalizedEdgeJson: JSON.stringify(normalizeCases[1].value),
    normalizedStringJson: JSON.stringify(normalizeCases[2].value),
    defaultTerminalJson: JSON.stringify(definitions.PRESET_THEMES[0].terminal),
    blurTerminalJson: JSON.stringify(
      terminalPalette.withWindowBlur(definitions.PRESET_THEMES[0].terminal, true)
    ),
  },
  helpers: {
    jsTrim: [
      "",
      " \t\n\r\u000B\f ",
      "\u00A0\u1680\u2000\u2001\u2002\u2003\u2004\u2005\u2006\u2007\u2008\u2009\u200A\u2028\u2029\u202F\u205F\u3000\uFEFF middle \u00A0",
      "\u200Bzero-width\u200B",
      "  中文😀e\u0301  ",
    ].map((value) => ({ value, trimmed: value.trim() })),
    jsNumber: [
      "",
      "   ",
      " 0x10 ",
      "0b101",
      "0o17",
      "Infinity",
      "-Infinity",
      "+Infinity",
      "1e3",
      ".5",
      "1.",
      "Infinityx",
      "NaN",
      "-0",
      "1_000",
      " 1 2 ",
      "0x",
      "0b2",
      "\u00A0 3 \u3000",
    ].map((value) => ({ value, number: String(Number(value)) })),
    jsRound: [-2.5, -1.5, -0.5, 0.5, 1.5, 2.5, NaN, Infinity, -Infinity].map((value) => ({
      value: Number.isNaN(value) ? "NaN" : String(value),
      rounded: String(Math.round(value)),
    })),
    jsNumberToString: [
      0,
      -0,
      1e-7,
      1e-6,
      1e20,
      1e21,
      1.2345678901234568e20,
      222.20000000000005,
      0.1 + 0.2,
      1.005,
      Number.MAX_VALUE,
      5e-324,
      -1.5,
    ].map((value) => ({
      input: Object.is(value, -0) ? "-0" : String(value),
      string: String(value),
    })),
  },
}

process.stdout.write(`${JSON.stringify(output, null, 2)}\n`)
