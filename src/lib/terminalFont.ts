import { getDetectedPlatform } from "@/lib/uiPlatform"

/**
 * The terminal font a first launch uses, by platform. A Nerd Font comes first
 * so prompts such as starship and powerlevel10k draw their icons, then each
 * system's own monospace font. These must match the backend's
 * `default_font_family`, which a backend test checks, because a first launch
 * reads its config from there.
 */
const DEFAULT_FONT_FAMILIES = {
  macos:
    '"JetBrains Mono Nerd Font", "JetBrainsMono Nerd Font", "JetBrains Mono", Menlo, Monaco, monospace',
  windows:
    '"JetBrains Mono Nerd Font", "JetBrainsMono Nerd Font", "JetBrains Mono", "Cascadia Code", Consolas, monospace',
  linux:
    '"JetBrains Mono Nerd Font", "JetBrainsMono Nerd Font", "JetBrains Mono", "DejaVu Sans Mono", monospace',
} as const

/** The default terminal font chain for a platform; any other platform gets Linux's. */
export function defaultTerminalFontFamily(platform: string = getDetectedPlatform()): string {
  return platform === "macos" || platform === "windows"
    ? DEFAULT_FONT_FAMILIES[platform]
    : DEFAULT_FONT_FAMILIES.linux
}

/**
 * The first font in a CSS font chain that the system has, which is the one the
 * terminal draws with; the whole chain when none of it is installed.
 */
export function firstInstalledFont(fontFamily: string, systemFonts: readonly string[]): string {
  const installed = new Set(systemFonts.map((font) => font.toLowerCase()))
  const names = fontFamily.split(",").map((name) => name.trim().replace(/^["']|["']$/g, ""))
  return names.find((name) => installed.has(name.toLowerCase())) ?? fontFamily
}
