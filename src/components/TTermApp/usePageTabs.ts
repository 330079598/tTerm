import type { i18n as I18n } from "i18next"
import { useEffect } from "react"

import type { AppShell } from "@/components/TTermApp/appShell"
import type { AppConfig } from "@/lib/appConfig"
import { handleAppPageRequests } from "@/lib/appNavigation"

interface PageTabsOptions {
  isLoaded: boolean
  config: AppConfig
  i18n: I18n
  settingsTabTitle: string
  tunnelsTabTitle: string
  logsTabTitle: string
}

/**
 * Keeps the UI in the configured language and the page tabs' titles in it,
 * and opens a page another window asks for.
 */
export function usePageTabs(shell: AppShell, options: PageTabsOptions) {
  const { renameSettingsTab, renameTunnelsTab, openLogsTab, renameLogsTab } = shell
  const { isLoaded, config, i18n, settingsTabTitle, tunnelsTabTitle, logsTabTitle } = options

  useEffect(() => {
    // i18next re-emits `languageChanged` (forcing every useTranslation()
    // consumer to re-render) even when called with the language it's
    // already on, so guard this to avoid a redundant second app-wide
    // re-render right after SettingsDialog's own changeLanguage call.
    if (isLoaded && i18n.language !== config.language) {
      i18n.changeLanguage(config.language)
    }
  }, [isLoaded, config.language, i18n])

  useEffect(() => {
    renameSettingsTab(settingsTabTitle)
  }, [renameSettingsTab, settingsTabTitle])

  useEffect(() => {
    renameTunnelsTab(tunnelsTabTitle)
  }, [renameTunnelsTab, tunnelsTabTitle])

  useEffect(() => {
    renameLogsTab(logsTabTitle)
  }, [renameLogsTab, logsTabTitle])

  useEffect(
    () =>
      handleAppPageRequests((page) => {
        if (page === "terminal-logs") openLogsTab(logsTabTitle)
      }),
    [logsTabTitle, openLogsTab]
  )
}
