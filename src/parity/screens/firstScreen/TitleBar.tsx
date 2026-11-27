import { useEffect, type CSSProperties, type ReactNode } from "react"
import { useTranslation } from "react-i18next"

import { TitleBarActions } from "@/components/TTermApp/TitleBarActions"
import { TitleBarTabs } from "@/components/TTermApp/TitleBarTabs"
import { useRuntimePlatform } from "@/components/TTermApp/useAppChrome"
import { useWindowControls } from "@/hooks/useWindowControls"
import { useConfig } from "@/contexts/ConfigContext"
import type { ParityScreenProps } from "@/parity/types"
import {
  firstScreenActiveTabId,
  firstScreenGroupsForTabs,
  firstScreenTabs,
} from "@/parity/screens/firstScreen/fixture"

/** The production title bar over a synthetic tab, with no window commands. */
export function FirstScreenTitleBar({
  children,
  onReady,
}: ParityScreenProps & { children: ReactNode }) {
  const { config, isLoaded } = useConfig()
  const { t } = useTranslation()
  const { isLinux, isMacos } = useRuntimePlatform()
  const {
    nativeControlsReservePx,
    handleMinimizeWindow,
    handleToggleMaximizeWindow,
    handleCloseWindow,
  } = useWindowControls(!isMacos)

  useEffect(() => {
    if (isLoaded) onReady()
  }, [isLoaded, onReady])

  return (
    <div
      className={`app ${isMacos ? "macos" : ""} ${isLinux ? "linux" : ""}`}
      data-tab-width-mode={config.tab_width_mode}
      style={
        {
          "--tab-standard-width": `${config.tab_standard_width}px`,
        } as CSSProperties
      }
    >
      <div className="title-bar" data-parity="workspace.first-screen.titlebar">
        <TitleBarTabs
          tabs={firstScreenTabs}
          groups={firstScreenGroupsForTabs}
          activeTabId={firstScreenActiveTabId}
          setActiveTab={() => {}}
          handleCloseTab={async () => {}}
          handleNewTab={() => {}}
          moveTab={() => {}}
          handleTabDragMove={() => {}}
          handleTabDrop={() => false}
          handleTabDragCancel={() => {}}
          handleTabContextMenu={() => {}}
          setShowProfilesPanel={() => {}}
          onOpenCommandLibrary={() => {}}
        />
        <div className="drag-space" data-tauri-drag-region />
        <TitleBarActions
          nativeControlsReservePx={nativeControlsReservePx}
          runningTunnelCount={0}
          settingsTabTitle={t("settings.title")}
          tunnelsButtonLabel={t("tunnels.title")}
          isLinux={isLinux}
          onTunnelsClick={() => {}}
          onSettingsClick={() => {}}
          onMinimizeWindow={handleMinimizeWindow}
          onToggleMaximizeWindow={handleToggleMaximizeWindow}
          onCloseWindow={handleCloseWindow}
        />
      </div>
      <div className="content-area" data-parity="workspace.first-screen.content">
        {children}
      </div>
    </div>
  )
}
