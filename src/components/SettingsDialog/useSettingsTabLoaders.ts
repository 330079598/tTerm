import { useEffect, useRef, type RefObject } from "react"
import { useTranslation } from "react-i18next"

import {
  getPerfNow,
  loadSystemFontsCached,
  logPerf,
  refreshSecretStatusCached,
} from "@/components/SettingsDialog/settingsLoaders"
import type { SecretBackendStatus } from "@/lib/appConfig"

interface SettingsTabLoadersOptions {
  activeTab: string
  isMountedRef: RefObject<boolean>
  fontsLoaded: boolean
  setSystemFonts: (fonts: string[]) => void
  setFontLoadError: (error: string | null) => void
  setFontsLoaded: (loaded: boolean) => void
  refreshSecretStatus: () => Promise<SecretBackendStatus>
}

/**
 * Loads what a settings page needs when it is first shown: the system fonts
 * for the terminal page and the saved-password status for the security page.
 * Also times how long the settings took to open.
 */
export function useSettingsTabLoaders({
  activeTab,
  isMountedRef,
  fontsLoaded,
  setSystemFonts,
  setFontLoadError,
  setFontsLoaded,
  refreshSecretStatus,
}: SettingsTabLoadersOptions) {
  const { t } = useTranslation()
  const mountStartRef = useRef(getPerfNow())

  useEffect(() => {
    logPerf("settings.open", mountStartRef.current, `tab=${activeTab}`)
  }, [activeTab])

  useEffect(() => {
    if (activeTab !== "terminal" || fontsLoaded) {
      return
    }

    // loadSystemFontsCached shares one in-flight request, so re-running this
    // (e.g. leaving and re-entering the tab mid-load) doesn't refetch.
    loadSystemFontsCached()
      .then((fonts) => {
        if (!isMountedRef.current) {
          return
        }
        setSystemFonts(fonts)
      })
      .catch(() => {
        if (!isMountedRef.current) {
          return
        }
        setSystemFonts([])
        setFontLoadError(
          t("fontSettings.loadingFontsFailed", {
            defaultValue:
              "Couldn't load the system font list. You can still enter a font name manually.",
          })
        )
      })
      .finally(() => {
        if (!isMountedRef.current) {
          return
        }
        setFontsLoaded(true)
      })
  }, [activeTab, fontsLoaded, isMountedRef, t, setFontLoadError, setFontsLoaded, setSystemFonts])

  useEffect(() => {
    if (activeTab !== "security") {
      return
    }

    refreshSecretStatusCached(refreshSecretStatus)
  }, [activeTab, refreshSecretStatus])

  return { loadingFonts: activeTab === "terminal" && !fontsLoaded }
}
