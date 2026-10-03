import {
  colorToHex,
  contrastRatio,
  hexToRgb,
  relativeLuminance,
  rgbToHex,
} from "@/components/ThemeEditor/colorUtils"
import type { TerminalPalette, ThemeColors } from "@/types/theme"

/**
 * A terminal color scheme as other terminals describe one: the 16 ANSI
 * colors plus the special colors. Theme files and the theme library both
 * come in this shape.
 */
export interface TerminalScheme {
  name?: string
  background: string
  foreground: string
  /** ANSI colors 0-15. */
  palette: string[]
  cursor?: string
  cursorText?: string
  selectionBackground?: string
  selectionForeground?: string
}

const ANSI_KEYS = [
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
] as const satisfies ReadonlyArray<keyof TerminalPalette>

const WHITE = "#ffffff"
const BLACK = "#000000"

function hex(value: string | undefined, fallback: string): string {
  return (value && colorToHex(value)) || fallback
}

function mix(from: string, to: string, amount: number): string {
  const a = hexToRgb(from)!
  const b = hexToRgb(to)!
  return rgbToHex(
    a.r + (b.r - a.r) * amount,
    a.g + (b.g - a.g) * amount,
    a.b + (b.b - a.b) * amount
  )
}

function contrast(first: string, second: string): number {
  return contrastRatio(first, second) ?? 1
}

/**
 * An HSL token precise enough to come back as the same hex color, so the
 * interface background matches the terminal's exactly.
 */
function hslToken(color: string): string {
  const { r, g, b } = hexToRgb(color)!
  const [red, green, blue] = [r / 255, g / 255, b / 255]
  const max = Math.max(red, green, blue)
  const min = Math.min(red, green, blue)
  const lightness = (max + min) / 2
  const delta = max - min
  let hue = 0
  let saturation = 0
  if (delta > 0) {
    saturation = delta / (1 - Math.abs(2 * lightness - 1))
    if (max === red) hue = ((green - blue) / delta + 6) % 6
    else if (max === green) hue = (blue - red) / delta + 2
    else hue = (red - green) / delta + 4
  }
  const round = (value: number) => Math.round(value * 100) / 100
  return `${round(hue * 60)} ${round(saturation * 100)}% ${round(lightness * 100)}%`
}

/** How colorful a color is, 0 for grays to 1 for pure hues. */
function chroma(color: string): number {
  const { r, g, b } = hexToRgb(color)!
  return (Math.max(r, g, b) - Math.min(r, g, b)) / 255
}

/**
 * `color`, moved toward `target` until it reaches `ratio` against
 * `background`. By default the target is white or black, whichever stands
 * out more.
 */
function ensureContrast(
  color: string,
  background: string,
  ratio: number,
  target = contrast(WHITE, background) >= contrast(BLACK, background) ? WHITE : BLACK
): string {
  if (contrast(color, background) >= ratio) return color
  for (let step = 1; step <= 20; step++) {
    const candidate = mix(color, target, step / 20)
    if (contrast(candidate, background) >= ratio) return candidate
  }
  return target
}

/** Text for a fill: the theme's own text or background color when either reads well. */
function textOn(fill: string, foreground: string, background: string): string {
  const [best] = [foreground, background].sort((a, b) => contrast(b, fill) - contrast(a, fill))
  if (contrast(best, fill) >= 4.5) return best
  return contrast(WHITE, fill) >= contrast(BLACK, fill) ? WHITE : BLACK
}

/** The scheme's most fitting accent: its blue, unless that is gray or hard to see. */
function pickPrimary(palette: TerminalPalette, background: string): string {
  const candidates = [
    palette.blue,
    palette.brightBlue,
    palette.cursor,
    palette.magenta,
    palette.cyan,
    palette.brightMagenta,
    palette.brightCyan,
  ].map((color) => hex(color, BLACK))
  const fitting = candidates.find(
    (color) => chroma(color) >= 0.2 && contrast(color, background) >= 2.5
  )
  if (fitting) return fitting

  const [mostColorful] = ANSI_KEYS.filter((key) => !/black|white/i.test(key))
    .map((key) => hex(palette[key], BLACK))
    .sort((a, b) => chroma(b) - chroma(a))
  return ensureContrast(mostColorful, background, 3)
}

export function isDarkPalette(palette: Pick<TerminalPalette, "background" | "foreground">) {
  const background = relativeLuminance(hex(palette.background, BLACK)) ?? 0
  const foreground = relativeLuminance(hex(palette.foreground, WHITE)) ?? 1
  return background < foreground
}

export function terminalPaletteFromScheme(scheme: TerminalScheme): TerminalPalette {
  const background = hex(scheme.background, BLACK)
  const foreground = hex(scheme.foreground, WHITE)
  const { r, g, b } = hexToRgb(foreground)!
  const palette: TerminalPalette = {
    background,
    foreground,
    cursor: hex(scheme.cursor, foreground),
    cursorAccent: hex(scheme.cursorText, background),
    selectionBackground: scheme.selectionBackground
      ? hex(scheme.selectionBackground, foreground)
      : `rgba(${r}, ${g}, ${b}, 0.25)`,
    black: "",
    red: "",
    green: "",
    yellow: "",
    blue: "",
    magenta: "",
    cyan: "",
    white: "",
    brightBlack: "",
    brightRed: "",
    brightGreen: "",
    brightYellow: "",
    brightBlue: "",
    brightMagenta: "",
    brightCyan: "",
    brightWhite: "",
  }
  ANSI_KEYS.forEach((key, index) => {
    palette[key] = hex(scheme.palette[index], index < 8 ? background : foreground)
  })
  if (scheme.selectionForeground) {
    palette.selectionForeground = hex(scheme.selectionForeground, foreground)
  }
  return palette
}

/**
 * Interface colors that go with a terminal palette, for themes that only
 * come with one. Surfaces step from the background toward the foreground,
 * the accent is the palette's blue and the status colors are its red, green
 * and yellow, each kept readable against the background.
 */
export function deriveThemeColors(palette: TerminalPalette): ThemeColors {
  const background = hex(palette.background, BLACK)
  const paletteForeground = hex(palette.foreground, WHITE)
  // Light surfaces need less of the (dark) text color to stand apart.
  const depth = isDarkPalette({ background, foreground: paletteForeground }) ? 1 : 0.6
  const surface = (amount: number) => mix(background, paletteForeground, amount * depth)
  const muted = surface(0.13)
  // Text sits on every surface up to the muted one; it may only get further
  // from the background, never cross over to its other side.
  const away = depth === 1 ? WHITE : BLACK
  const foreground = ensureContrast(
    ensureContrast(paletteForeground, background, 4.5, away),
    muted,
    4.5,
    away
  )

  const primary = pickPrimary(palette, background)
  const red = ensureContrast(hex(palette.red, "#e5534b"), background, 3)
  const green = ensureContrast(hex(palette.green, "#57ab5a"), background, 3)
  const yellow = ensureContrast(hex(palette.yellow, "#c69026"), background, 3)
  const statusText = (color: string) =>
    ensureContrast(mix(color, foreground, 0.35), background, 4.5)

  // A tint of the accent for hovered and selected items, light enough to read on.
  let tint = 0.22
  while (tint > 0.04 && contrast(foreground, mix(background, primary, tint)) < 4.5) tint -= 0.03
  const accent = mix(background, primary, tint)

  const colors = {
    background,
    foreground,
    card: surface(0.06),
    cardForeground: foreground,
    popover: surface(0.07),
    popoverForeground: foreground,
    primary,
    primaryForeground: textOn(primary, foreground, background),
    secondary: surface(0.08),
    secondaryForeground: foreground,
    muted,
    mutedForeground: ensureContrast(mix(foreground, background, 0.35), surface(0.08), 4.5),
    accent,
    accentForeground: foreground,
    destructive: red,
    destructiveForeground: textOn(red, foreground, background),
    success: green,
    successForeground: statusText(green),
    warning: yellow,
    warningForeground: statusText(yellow),
    border: surface(0.16),
    input: surface(0.18),
    ring: primary,
    tabBackground: background,
    tabActive: surface(0.1),
    tabActiveBorder: primary,
    tabHover: surface(0.14),
    titlebar: surface(0.04),
  } satisfies ThemeColors

  return Object.fromEntries(
    Object.entries(colors).map(([key, value]) => [key, hslToken(value)])
  ) as unknown as ThemeColors
}
