import { deriveThemeColors, isDarkPalette, terminalPaletteFromScheme } from "@/lib/themeDerivation"
import type { TerminalScheme } from "@/lib/themeDerivation"
import type { CatalogTheme, TerminalPalette, Theme, ThemeColors } from "@/types/theme"

/**
 * The theme library: the schemes of iTerm2-Color-Schemes, which Ghostty also
 * ships, bundled from src/assets/themes by scripts/update-themes.mjs. They
 * are loaded on demand and never saved; the configured theme names one by
 * id and other devices resolve it from their own copy.
 */

export const THEME_CATALOG_SOURCE = "https://github.com/mbadolato/iTerm2-Color-Schemes"

const CATALOG_ID_PREFIX = "catalog:"

interface CatalogFile {
  commit: string
  themes: Array<TerminalScheme & { name: string }>
}

export function isCatalogThemeId(themeId: string): boolean {
  return themeId.startsWith(CATALOG_ID_PREFIX)
}

export function isCatalogTheme(theme: Theme | undefined): theme is CatalogTheme {
  return !!theme && "isCatalog" in theme
}

export function catalogThemeName(themeId: string): string {
  return themeId.slice(CATALOG_ID_PREFIX.length)
}

/**
 * A library theme from its terminal palette. Its interface colors are
 * derived on first use: of the hundreds of themes in the library, only the
 * few that get previewed or chosen ever need them.
 */
export function createCatalogTheme(themeId: string, terminal: TerminalPalette): CatalogTheme {
  let colors: ThemeColors | undefined
  return {
    id: themeId,
    name: catalogThemeName(themeId),
    get colors() {
      return (colors ??= deriveThemeColors(terminal))
    },
    terminal,
    isCustom: false,
    isCatalog: true,
    isDark: isDarkPalette(terminal),
  }
}

export function catalogThemeFromScheme(scheme: TerminalScheme & { name: string }): CatalogTheme {
  return createCatalogTheme(`${CATALOG_ID_PREFIX}${scheme.name}`, terminalPaletteFromScheme(scheme))
}

let catalog: Promise<CatalogTheme[]> | null = null

export function loadThemeCatalog(): Promise<CatalogTheme[]> {
  catalog ??= import("@/assets/themes/catalog.json")
    .then((module) => (module.default as CatalogFile).themes.map(catalogThemeFromScheme))
    .catch((error: unknown) => {
      catalog = null
      throw error
    })
  return catalog
}
