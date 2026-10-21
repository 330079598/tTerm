import { useCallback, useMemo } from "react"

import type { AppConfig } from "@/lib/appConfig"
import { isWindowBlurEnabled } from "@/lib/uiPlatform"
import type { TerminalTabProps } from "@/components/TerminalTab/types"
import { withWindowBlur } from "@/lib/terminalPalette"
import { useCatalogFor, useTheme } from "@/contexts/ThemeContext"
import { useStableRef } from "@/hooks/useStableRef"

type UseTerminalThemeOptions = Pick<TerminalTabProps, "connection"> & { config: AppConfig }

/**
 * The terminal's colors: the connection's own theme, else the app theme,
 * adjusted for the window blur.
 */
export function useTerminalTheme({ connection, config }: UseTerminalThemeOptions) {
  const { displayedTheme, getTheme } = useTheme()
  const windowBlur = isWindowBlurEnabled(config)
  // A connection's own colors win over the app theme; one that no longer
  // exists falls back to the app theme.
  const connectionThemeId = connection?.type === "ssh" ? connection.terminalTheme : undefined
  // A library theme resolves once the library has loaded.
  useCatalogFor([connectionThemeId])
  const connectionPalette = connectionThemeId ? getTheme(connectionThemeId)?.terminal : undefined
  const resolveTerminalTheme = useCallback(
    () => ({
      ...withWindowBlur(
        connectionPalette ?? getTheme(displayedTheme)?.terminal ?? getTheme("default")!.terminal,
        windowBlur
      ),
      // The overview ruler under the scrollbar shows its marks only.
      overviewRulerBorder: "#00000000",
    }),
    [connectionPalette, displayedTheme, getTheme, windowBlur]
  )

  const terminalTheme = useMemo(() => resolveTerminalTheme(), [resolveTerminalTheme])
  const terminalThemeRef = useStableRef(terminalTheme)

  return {
    windowBlur,
    connectionPalette,
    resolveTerminalTheme,
    terminalTheme,
    terminalThemeRef,
  }
}
