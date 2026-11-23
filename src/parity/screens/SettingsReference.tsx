import "@/i18n/config"
import "@/components/TTermApp/Workspace.css"

import { useEffect } from "react"

import { SettingsPanel } from "@/components/SettingsDialog"
import { AppActivityProvider } from "@/contexts/AppActivityContext"
import { ConfigProvider, useConfig } from "@/contexts/ConfigContext"
import { ThemeProvider } from "@/contexts/ThemeContext"
import type { ParityScreenProps } from "@/parity/types"

/**
 * The settings page as a workspace page shows it. `SettingsPanel` is the
 * component `WorkspacePanel` renders for a `settings` tab; the wrapper gives
 * it the same full-height page surface. The fixture supplies the configuration
 * and commands, and the search query is typed by the capture actions rather
 * than seeded here.
 */
export function SettingsReference({ onReady }: ParityScreenProps) {
  return (
    <ConfigProvider>
      <ThemeProvider>
        <AppActivityProvider>
          <SettingsReferenceContent onReady={onReady} />
        </AppActivityProvider>
      </ThemeProvider>
    </ConfigProvider>
  )
}

function SettingsReferenceContent({ onReady }: ParityScreenProps) {
  const { isLoaded } = useConfig()

  useEffect(() => {
    if (isLoaded) onReady()
  }, [isLoaded, onReady])

  return (
    <div className="workspace-panel">
      <SettingsPanel defaultTab="general" />
    </div>
  )
}
