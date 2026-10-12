import { useCallback, useEffect, useMemo, useState } from "react"

import { createCatalogTheme, isCatalogThemeId, loadThemeCatalog } from "@/lib/themeCatalog"
import { readCachedTheme } from "@/lib/themePreloader"
import type { CatalogTheme } from "@/types/theme"

/**
 * The theme library: loaded when the configured theme comes from it and the
 * cached copy can't stand in, or when a page asks for it.
 */
export function useThemeCatalog(configuredThemeId: string) {
  const [catalogThemes, setCatalogThemes] = useState<CatalogTheme[] | null>(null)
  const [catalogFailed, setCatalogFailed] = useState(false)
  // The library theme the last session showed, rebuilt from its cached
  // palette so a start in it never loads the library.
  const [cachedCatalogTheme] = useState<CatalogTheme | null>(() => {
    const cache = readCachedTheme()
    return cache && isCatalogThemeId(cache.id) && cache.terminal
      ? createCatalogTheme(cache.id, cache.terminal)
      : null
  })

  const knownCatalogThemes = useMemo(
    () => catalogThemes ?? (cachedCatalogTheme ? [cachedCatalogTheme] : []),
    [cachedCatalogTheme, catalogThemes]
  )

  const loadCatalogThemes = useCallback(async () => {
    const themes = await loadThemeCatalog()
    setCatalogThemes(themes)
    setCatalogFailed(false)
    return themes
  }, [])

  // Only a library theme the cache cannot stand in for needs the library;
  // one the library lacks too shows the fallback once it has loaded.
  const needsCatalog =
    isCatalogThemeId(configuredThemeId) &&
    catalogThemes === null &&
    !knownCatalogThemes.some((theme) => theme.id === configuredThemeId)
  useEffect(() => {
    if (!needsCatalog) return
    // eslint-disable-next-line react-hooks/set-state-in-effect -- state is only set once the library loads or fails
    loadCatalogThemes().catch((error: unknown) => {
      console.error("Failed to load the theme library:", error)
      setCatalogFailed(true)
    })
  }, [loadCatalogThemes, needsCatalog])

  // Until then that theme is unknown.
  const catalogSettled = !needsCatalog || catalogFailed

  return { catalogThemes, knownCatalogThemes, loadCatalogThemes, catalogSettled }
}
