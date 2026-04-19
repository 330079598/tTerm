import type { TerminalPalette, ThemeColors } from "@/types/theme"

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value))
}

export function parseHsl(hsl: string): { h: number; s: number; l: number } | null {
  const match = hsl.trim().match(/(\d+\.?\d*)\s+(\d+\.?\d*)%?\s+(\d+\.?\d*)%?/)
  if (!match) return null

  return {
    h: parseFloat(match[1]),
    s: parseFloat(match[2]),
    l: parseFloat(match[3]),
  }
}

function hslToCss(hsl: { h: number; s: number; l: number }): string {
  return `hsl(${hsl.h} ${hsl.s}% ${hsl.l}%)`
}

function hslaToCss(hsl: { h: number; s: number; l: number }, alpha: number): string {
  return `hsl(${hsl.h} ${hsl.s}% ${hsl.l}% / ${alpha})`
}

function shift(
  base: { h: number; s: number; l: number },
  delta: Partial<{ h: number; s: number; l: number }>
) {
  return {
    h: (base.h + (delta.h ?? 0) + 360) % 360,
    s: clamp(base.s + (delta.s ?? 0), 0, 100),
    l: clamp(base.l + (delta.l ?? 0), 0, 100),
  }
}

function deriveSurface(colors: ThemeColors) {
  return parseHsl(colors.background) ?? { h: 220, s: 13, l: 12 }
}

function deriveForeground(colors: ThemeColors) {
  return parseHsl(colors.foreground) ?? { h: 0, s: 0, l: 88 }
}

function deriveAccent(colors: ThemeColors) {
  return parseHsl(colors.primary) ?? parseHsl(colors.accent) ?? { h: 207, s: 100, l: 40 }
}

function deriveSuccess(colors: ThemeColors) {
  return parseHsl(colors.success) ?? { h: 142, s: 72, l: 42 }
}

function deriveWarning(colors: ThemeColors) {
  return parseHsl(colors.warning) ?? { h: 38, s: 92, l: 52 }
}

function deriveDestructive(colors: ThemeColors) {
  return parseHsl(colors.destructive) ?? { h: 0, s: 84, l: 60 }
}

export function generateTerminalPaletteFromColors(colors: ThemeColors): TerminalPalette {
  const surface = deriveSurface(colors)
  const foreground = deriveForeground(colors)
  const accent = deriveAccent(colors)
  const success = deriveSuccess(colors)
  const warning = deriveWarning(colors)
  const destructive = deriveDestructive(colors)
  const neutral = parseHsl(colors.muted) ?? shift(surface, { l: foreground.l > 50 ? 12 : 18 })
  const neutralText =
    parseHsl(colors.mutedForeground) ?? shift(foreground, { l: foreground.l > 50 ? -18 : 18 })
  const cyanBase = parseHsl(colors.accent) ?? shift(accent, { h: -18, s: -8, l: 4 })
  const magentaBase = shift(accent, { h: 55, s: -6, l: 8 })
  const whiteBase = shift(foreground, { s: -10, l: foreground.l > 50 ? -6 : 8 })

  return {
    background: hslToCss(surface),
    foreground: hslToCss(foreground),
    cursor: hslToCss(shift(accent, { l: foreground.l > 50 ? -8 : 14 })),
    selectionBackground: hslaToCss(accent, 0.24),
    black: hslToCss(shift(surface, { l: foreground.l > 50 ? -2 : 8 })),
    red: hslToCss(destructive),
    green: hslToCss(success),
    yellow: hslToCss(warning),
    blue: hslToCss(accent),
    magenta: hslToCss(magentaBase),
    cyan: hslToCss(cyanBase),
    white: hslToCss(whiteBase),
    brightBlack: hslToCss(shift(neutral, { l: foreground.l > 50 ? -10 : 12 })),
    brightRed: hslToCss(shift(destructive, { l: foreground.l > 50 ? -8 : 10 })),
    brightGreen: hslToCss(shift(success, { l: foreground.l > 50 ? -6 : 10 })),
    brightYellow: hslToCss(shift(warning, { l: foreground.l > 50 ? -8 : 8 })),
    brightBlue: hslToCss(shift(accent, { l: foreground.l > 50 ? -8 : 12 })),
    brightMagenta: hslToCss(shift(magentaBase, { l: foreground.l > 50 ? -8 : 10 })),
    brightCyan: hslToCss(shift(cyanBase, { l: foreground.l > 50 ? -8 : 10 })),
    brightWhite: hslToCss(shift(neutralText, { l: foreground.l > 50 ? -28 : 22, s: -10 })),
  }
}
