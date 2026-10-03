import { colorToHex } from "@/components/ThemeEditor/colorUtils"
import type { TerminalScheme } from "@/lib/themeDerivation"

/**
 * Reads the color schemes other terminals save, so a theme made for one of
 * them can be brought over. Each reader collects what it finds and
 * `completeScheme` checks that the 16 colors and the background and
 * foreground are all there.
 */

export type ThemeFileFormat = "ghostty" | "windowsTerminal" | "iterm" | "alacritty" | "kitty"

export interface ImportedScheme extends TerminalScheme {
  name: string
}

export interface ThemeImport {
  format: ThemeFileFormat
  schemes: ImportedScheme[]
}

export class ThemeImportError extends Error {
  constructor(
    readonly reason: "unknownFormat" | "missingColors" | "invalidFile",
    readonly missing: string[] = []
  ) {
    super(reason === "missingColors" ? `Missing colors: ${missing.join(", ")}` : reason)
  }
}

interface PartialScheme {
  name?: string
  background?: string
  foreground?: string
  palette: Array<string | undefined>
  cursor?: string
  cursorText?: string
  selectionBackground?: string
  selectionForeground?: string
}

type SpecialColor = Exclude<keyof PartialScheme, "name" | "palette">

const ANSI_NAMES = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"]

/** A color as theme files write it: #rgb, #rrggbb, #rrggbbaa, 0xrrggbb or a CSS function. */
function parseColor(value: string | undefined): string | undefined {
  if (!value) return undefined
  const trimmed = value.trim().replace(/^['"]|['"]$/g, "")
  const hex = trimmed.match(/^(?:#|0x)?([\da-f]{6})(?:[\da-f]{2})?$/i)?.[1]
  return (hex ? `#${hex.toLowerCase()}` : colorToHex(trimmed)) ?? undefined
}

function emptyScheme(name?: string): PartialScheme {
  return { name, palette: [] }
}

function completeScheme(scheme: PartialScheme, fallbackName: string): ImportedScheme {
  const missing = [
    ...(scheme.background ? [] : ["background"]),
    ...(scheme.foreground ? [] : ["foreground"]),
    ...Array.from({ length: 16 }, (_, index) => index)
      .filter((index) => !scheme.palette[index])
      .map((index) => `color${index}`),
  ]
  if (missing.length > 0) throw new ThemeImportError("missingColors", missing)
  return {
    ...scheme,
    name: scheme.name?.trim() || fallbackName,
    background: scheme.background!,
    foreground: scheme.foreground!,
    palette: scheme.palette as string[],
  }
}

/** `key = value` lines; Ghostty writes the ANSI colors as `palette = N=#rrggbb`. */
function readGhostty(text: string): PartialScheme {
  const scheme = emptyScheme()
  const special: Record<string, SpecialColor> = {
    background: "background",
    foreground: "foreground",
    "cursor-color": "cursor",
    "cursor-text": "cursorText",
    "selection-background": "selectionBackground",
    "selection-foreground": "selectionForeground",
  }
  for (const line of text.split(/\r?\n/)) {
    const match = line.match(/^\s*([\w-]+)\s*=\s*(.*?)\s*$/)
    if (!match) continue
    const [, key, value] = match
    if (key === "palette") {
      const entry = value.match(/^(\d+)\s*=\s*(\S+)$/)
      if (entry && Number(entry[1]) < 16) scheme.palette[Number(entry[1])] = parseColor(entry[2])
    } else if (special[key]) {
      scheme[special[key]] = parseColor(value)
    }
  }
  return scheme
}

/** `key value` lines, with the name in a `## name:` comment as kitty's themes carry it. */
function readKitty(text: string): PartialScheme {
  const scheme = emptyScheme(text.match(/^\s*##\s*name\s*:\s*(.+?)\s*$/im)?.[1])
  const special: Record<string, SpecialColor> = {
    background: "background",
    foreground: "foreground",
    cursor: "cursor",
    cursor_text_color: "cursorText",
    selection_background: "selectionBackground",
    selection_foreground: "selectionForeground",
  }
  for (const line of text.split(/\r?\n/)) {
    const match = line.match(/^\s*(\w+)\s+(\S+)\s*$/)
    if (!match) continue
    const [, key, value] = match
    const index = key.match(/^color(\d+)$/)?.[1]
    if (index !== undefined && Number(index) < 16) scheme.palette[Number(index)] = parseColor(value)
    else if (special[key]) scheme[special[key]] = parseColor(value)
  }
  return scheme
}

/**
 * Alacritty's `[colors.primary]`-style TOML sections, or the `primary:`
 * blocks of its older YAML files.
 */
function readAlacritty(text: string): PartialScheme {
  const scheme = emptyScheme()
  let section = ""
  for (const line of text.split(/\r?\n/)) {
    const header =
      line.match(/^\s*\[colors\.(\w+)\]\s*$/)?.[1] ?? line.match(/^\s*(\w+)\s*:\s*$/)?.[1]
    if (header) {
      section = header
      continue
    }
    const match = line.match(/^\s*(\w+)\s*[=:]\s*(\S+)/)
    if (!match) continue
    const [, key, raw] = match
    const value = parseColor(raw)
    const index = ANSI_NAMES.indexOf(key)
    if (section === "primary" && (key === "background" || key === "foreground")) {
      scheme[key] = value
    } else if ((section === "normal" || section === "bright") && index !== -1) {
      scheme.palette[index + (section === "bright" ? 8 : 0)] = value
    } else if (section === "cursor") {
      if (key === "cursor") scheme.cursor = value
      if (key === "text") scheme.cursorText = value
    } else if (section === "selection") {
      if (key === "background") scheme.selectionBackground = value
      if (key === "text") scheme.selectionForeground = value
    }
  }
  return scheme
}

/** The components of each `<key>… Color</key><dict>…</dict>` entry of an .itermcolors plist. */
function readIterm(text: string): PartialScheme {
  const colors = new Map<string, string>()
  for (const [, key, body] of text.matchAll(/<key>([^<]+)<\/key>\s*<dict>([\s\S]*?)<\/dict>/g)) {
    const component = (name: string) =>
      Number(
        body.match(
          new RegExp(`<key>${name} Component</key>\\s*<(?:real|integer)>([^<]+)</`)
        )?.[1] ?? NaN
      )
    const [r, g, b] = ["Red", "Green", "Blue"].map(component)
    if ([r, g, b].some(Number.isNaN)) continue
    const channel = (value: number) =>
      Math.round(Math.min(1, Math.max(0, value)) * 255)
        .toString(16)
        .padStart(2, "0")
    colors.set(key.trim(), `#${channel(r)}${channel(g)}${channel(b)}`)
  }
  // iTerm2 3.5 saves a color per appearance; the dark one stands in for the plain one.
  const color = (key: string) => colors.get(key) ?? colors.get(`${key} (Dark)`)
  const scheme = emptyScheme()
  for (let index = 0; index < 16; index++) scheme.palette[index] = color(`Ansi ${index} Color`)
  scheme.background = color("Background Color")
  scheme.foreground = color("Foreground Color")
  scheme.cursor = color("Cursor Color")
  scheme.cursorText = color("Cursor Text Color")
  scheme.selectionBackground = color("Selection Color")
  scheme.selectionForeground = color("Selected Text Color")
  return scheme
}

/** JSON with comments and trailing commas, as Windows Terminal's settings.json allows. */
function parseJsonc(text: string): unknown {
  let result = ""
  for (let index = 0; index < text.length; index++) {
    const char = text[index]
    if (char === '"') {
      const end = text.slice(index).match(/^"(?:\\.|[^"\\])*"/)?.[0] ?? char
      result += end
      index += end.length - 1
    } else if (text.startsWith("//", index)) {
      index = text.indexOf("\n", index) === -1 ? text.length : text.indexOf("\n", index) - 1
    } else if (text.startsWith("/*", index)) {
      index = text.indexOf("*/", index) === -1 ? text.length : text.indexOf("*/", index) + 1
    } else {
      result += char
    }
  }
  return JSON.parse(result.replace(/,(\s*[}\]])/g, "$1"))
}

/** A Windows Terminal scheme, a list of them, or a settings.json that holds them. */
function readWindowsTerminal(json: unknown): PartialScheme[] {
  const found = Array.isArray(json)
    ? json
    : json && typeof json === "object" && Array.isArray((json as { schemes?: unknown }).schemes)
      ? (json as { schemes: unknown[] }).schemes
      : [json]
  return found
    .filter((entry): entry is Record<string, unknown> => !!entry && typeof entry === "object")
    .map((entry) => {
      const value = (key: string) =>
        typeof entry[key] === "string" ? parseColor(entry[key] as string) : undefined
      const scheme = emptyScheme(typeof entry.name === "string" ? entry.name : undefined)
      ANSI_NAMES.forEach((name, index) => {
        // Windows Terminal calls magenta purple.
        const alias = name === "magenta" ? "purple" : name
        const bright = `bright${alias[0].toUpperCase()}${alias.slice(1)}`
        scheme.palette[index] = value(alias) ?? value(name)
        scheme.palette[index + 8] =
          value(bright) ?? value(`bright${name[0].toUpperCase()}${name.slice(1)}`)
      })
      scheme.background = value("background")
      scheme.foreground = value("foreground")
      scheme.cursor = value("cursorColor")
      scheme.selectionBackground = value("selectionBackground")
      return scheme
    })
}

function detectFormat(text: string): ThemeFileFormat | null {
  const trimmed = text.trim()
  // Before JSON: a TOML file can open with `[` too.
  if (
    /^\s*\[colors\.\w+\]/m.test(trimmed) ||
    /^\s*(primary|normal|bright)\s*:\s*$/m.test(trimmed)
  ) {
    return "alacritty"
  }
  if (trimmed.startsWith("{") || trimmed.startsWith("[")) return "windowsTerminal"
  if (/<plist[\s>]/.test(trimmed) || /<key>Ansi 0 Color/.test(trimmed)) return "iterm"
  if (/^\s*palette\s*=\s*\d+\s*=/m.test(trimmed)) return "ghostty"
  if (/^\s*color\d+\s+\S+/m.test(trimmed)) return "kitty"
  return null
}

/**
 * The schemes in a theme file. `fileName` names a scheme the file leaves
 * unnamed; several schemes only come from a Windows Terminal settings file.
 */
export function parseThemeFile(text: string, fileName = ""): ThemeImport {
  const format = detectFormat(text)
  if (!format) throw new ThemeImportError("unknownFormat")
  const fallbackName = fileName.replace(/\.[^.]+$/, "").trim() || "Imported theme"

  if (format === "windowsTerminal") {
    let json: unknown
    try {
      json = parseJsonc(text)
    } catch {
      throw new ThemeImportError("invalidFile")
    }
    const schemes = readWindowsTerminal(json)
    const complete = schemes.flatMap((scheme) => {
      try {
        return [completeScheme(scheme, fallbackName)]
      } catch {
        return []
      }
    })
    // Report what the first scheme lacks when none is usable.
    if (complete.length === 0) completeScheme(schemes[0] ?? emptyScheme(), fallbackName)
    return { format, schemes: complete }
  }

  const read = {
    ghostty: readGhostty,
    kitty: readKitty,
    alacritty: readAlacritty,
    iterm: readIterm,
  }
  return { format, schemes: [completeScheme(read[format](text), fallbackName)] }
}
