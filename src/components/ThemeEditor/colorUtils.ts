import type { TerminalPalette, ThemeColors } from "@/types/theme"

import type { HslParts } from "@/components/ThemeEditor/types"

export function isHslValue(value: string): boolean {
  return /^\d+(\.\d+)?\s+\d+(\.\d+)?%\s+\d+(\.\d+)?%$/.test(value.trim())
}

export function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value))
}

function componentToHex(value: number): string {
  return Math.round(clamp(value, 0, 255))
    .toString(16)
    .padStart(2, "0")
}

export function rgbToHex(r: number, g: number, b: number): string {
  return `#${componentToHex(r)}${componentToHex(g)}${componentToHex(b)}`
}

export function hexToRgb(hex: string): { r: number; g: number; b: number } | null {
  const normalized = hex.trim().replace(/^#/, "")
  const expanded =
    normalized.length === 3
      ? normalized
          .split("")
          .map((char) => char + char)
          .join("")
      : normalized

  if (!/^[\da-f]{6}$/i.test(expanded)) {
    return null
  }

  return {
    r: parseInt(expanded.slice(0, 2), 16),
    g: parseInt(expanded.slice(2, 4), 16),
    b: parseInt(expanded.slice(4, 6), 16),
  }
}

export function hslToRgb(h: number, s: number, l: number): { r: number; g: number; b: number } {
  const hue = (((h % 360) + 360) % 360) / 360
  const saturation = clamp(s, 0, 100) / 100
  const lightness = clamp(l, 0, 100) / 100

  if (saturation === 0) {
    const gray = lightness * 255
    return { r: gray, g: gray, b: gray }
  }

  const q =
    lightness < 0.5 ? lightness * (1 + saturation) : lightness + saturation - lightness * saturation
  const p = 2 * lightness - q
  const hueToRgb = (t: number) => {
    let shifted = t
    if (shifted < 0) shifted += 1
    if (shifted > 1) shifted -= 1
    if (shifted < 1 / 6) return p + (q - p) * 6 * shifted
    if (shifted < 1 / 2) return q
    if (shifted < 2 / 3) return p + (q - p) * (2 / 3 - shifted) * 6
    return p
  }

  return {
    r: hueToRgb(hue + 1 / 3) * 255,
    g: hueToRgb(hue) * 255,
    b: hueToRgb(hue - 1 / 3) * 255,
  }
}

function rgbToHslToken(r: number, g: number, b: number): string {
  const { h, s, l } = rgbToHsl(r, g, b)
  return `${h} ${s}% ${l}%`
}

export function hexToHslToken(hex: string): string | null {
  const rgb = hexToRgb(hex)
  return rgb ? rgbToHslToken(rgb.r, rgb.g, rgb.b) : null
}

function parseHslParts(value: string): HslParts | null {
  const normalized = value.trim()
  const tokenMatch = normalized.match(/^(\d+(?:\.\d+)?)\s+(\d+(?:\.\d+)?)%\s+(\d+(?:\.\d+)?)%$/)
  const functionMatch = normalized.match(
    /^hsla?\(\s*(\d+(?:\.\d+)?)(?:deg)?[\s,]+(\d+(?:\.\d+)?)%[\s,]+(\d+(?:\.\d+)?)%/i
  )
  const match = tokenMatch ?? functionMatch

  if (!match) return null

  return {
    h: Number(match[1]),
    s: Number(match[2]),
    l: Number(match[3]),
  }
}

export function colorToHex(value: string): string | null {
  const normalized = value.trim()

  if (/^#[\da-f]{3}$/i.test(normalized) || /^#[\da-f]{6}$/i.test(normalized)) {
    const rgb = hexToRgb(normalized)
    return rgb ? rgbToHex(rgb.r, rgb.g, rgb.b) : null
  }

  const rgbMatch = normalized.match(
    /^rgba?\(\s*(\d+(?:\.\d+)?)[\s,]+(\d+(?:\.\d+)?)[\s,]+(\d+(?:\.\d+)?)/i
  )
  if (rgbMatch) {
    return rgbToHex(Number(rgbMatch[1]), Number(rgbMatch[2]), Number(rgbMatch[3]))
  }

  const hsl = parseHslParts(normalized)
  if (hsl) {
    const rgb = hslToRgb(hsl.h, hsl.s, hsl.l)
    return rgbToHex(rgb.r, rgb.g, rgb.b)
  }

  return null
}

function rgbToHsl(r: number, g: number, b: number): HslParts {
  const red = clamp(r, 0, 255) / 255
  const green = clamp(g, 0, 255) / 255
  const blue = clamp(b, 0, 255) / 255
  const max = Math.max(red, green, blue)
  const min = Math.min(red, green, blue)
  const lightness = (max + min) / 2

  if (max === min) {
    return { h: 0, s: 0, l: Math.round(lightness * 100) }
  }

  const delta = max - min
  const saturation = lightness > 0.5 ? delta / (2 - max - min) : delta / (max + min)
  let hue = 0

  if (max === red) {
    hue = (green - blue) / delta + (green < blue ? 6 : 0)
  } else if (max === green) {
    hue = (blue - red) / delta + 2
  } else {
    hue = (red - green) / delta + 4
  }

  return {
    h: Math.round(hue * 60),
    s: Math.round(saturation * 100),
    l: Math.round(lightness * 100),
  }
}

export function hexToHslParts(hex: string): HslParts | null {
  const rgb = hexToRgb(hex)
  return rgb ? rgbToHsl(rgb.r, rgb.g, rgb.b) : null
}

export function readableTextHex(backgroundHex: string): string {
  const rgb = hexToRgb(backgroundHex)
  if (!rgb) return "#ffffff"
  const luminance = (0.299 * rgb.r + 0.587 * rgb.g + 0.114 * rgb.b) / 255
  return luminance > 0.58 ? "#111827" : "#ffffff"
}

function relativeLuminance(hex: string): number | null {
  const rgb = hexToRgb(hex)
  if (!rgb) return null
  const channel = (value: number) => {
    const normalized = value / 255
    return normalized <= 0.03928 ? normalized / 12.92 : Math.pow((normalized + 0.055) / 1.055, 2.4)
  }
  return 0.2126 * channel(rgb.r) + 0.7152 * channel(rgb.g) + 0.0722 * channel(rgb.b)
}

export function contrastRatio(first: string, second: string): number | null {
  const firstLum = relativeLuminance(first)
  const secondLum = relativeLuminance(second)
  if (firstLum === null || secondLum === null) return null
  const lighter = Math.max(firstLum, secondLum)
  const darker = Math.min(firstLum, secondLum)
  return (lighter + 0.05) / (darker + 0.05)
}

export function normalizeColorPreview(value: string): string {
  const normalized = value.trim()

  if (
    normalized.startsWith("#") ||
    normalized.startsWith("rgb(") ||
    normalized.startsWith("rgba(") ||
    normalized.startsWith("hsl(") ||
    normalized.startsWith("hsla(")
  ) {
    return normalized
  }

  if (isHslValue(normalized)) {
    return `hsl(${normalized})`
  }

  return normalized
}

export function resolveCssColor(value: string): string {
  return normalizeColorPreview(value)
}

export function createThemeSuggestions(colors: ThemeColors): string[] {
  const values = [
    colors.primary,
    colors.accent,
    colors.success,
    colors.warning,
    colors.destructive,
    colors.background,
    colors.foreground,
    colors.card,
    colors.muted,
    colors.border,
  ]

  return Array.from(
    new Set(values.map(colorToHex).filter((value): value is string => Boolean(value)))
  )
}

export function createTerminalSuggestions(
  colors: ThemeColors,
  terminal: TerminalPalette
): string[] {
  const values = [
    terminal.background,
    terminal.foreground,
    terminal.blue,
    terminal.green,
    terminal.red,
    terminal.yellow,
    terminal.magenta,
    terminal.cyan,
    ...createThemeSuggestions(colors),
  ]

  return Array.from(
    new Set(values.map(colorToHex).filter((value): value is string => Boolean(value)))
  )
}
