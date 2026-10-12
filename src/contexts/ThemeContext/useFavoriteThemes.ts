import { useCallback, useEffect, useRef } from "react"

import type { AppConfig } from "@/lib/appConfig"

/** Stars and unstars themes in `favorite_themes`. */
export function useFavoriteThemes(
  favoriteThemes: string[],
  saveConfig: (update: Partial<AppConfig>) => Promise<void>
) {
  // Starring twice before the first save lands must not lose a star.
  const favoritesRef = useRef(favoriteThemes)
  useEffect(() => {
    favoritesRef.current = favoriteThemes
  }, [favoriteThemes])

  return useCallback(
    async (themeId: string) => {
      const previous = favoritesRef.current
      favoritesRef.current = previous.includes(themeId)
        ? previous.filter((id) => id !== themeId)
        : [...previous, themeId]
      try {
        await saveConfig({ favorite_themes: favoritesRef.current })
      } catch (error) {
        favoritesRef.current = previous
        throw error
      }
    },
    [saveConfig]
  )
}
