import { useEffect, type CSSProperties } from "react"
import { useTranslation } from "react-i18next"

import { ConfigProvider, useConfig } from "@/contexts/ConfigContext"
import { TitleBarActions } from "@/components/TTermApp/TitleBarActions"
import { TitleBarTabs } from "@/components/TTermApp/TitleBarTabs"
import { useWindowControls } from "@/hooks/useWindowControls"
import type { ParityScreenProps } from "@/parity/types"
import {
  workspaceTitleBarActiveTabId,
  workspaceTitleBarGroups,
  workspaceTitleBarTabs,
} from "@/parity/screens/workspaceTitleBarFixture"

/**
 * The production title bar's Windows branch with two local shells. It uses
 * the same tab strip and action group as `AppTitleBar`, without the broadcast,
 * transfer and page hooks that the capture cannot exercise.
 */
export function WorkspaceTitleBar({ onReady }: ParityScreenProps) {
  return (
    <ConfigProvider>
      <WorkspaceTitleBarContent onReady={onReady} />
    </ConfigProvider>
  )
}

function WorkspaceTitleBarContent({ onReady }: ParityScreenProps) {
  const { config, isLoaded } = useConfig()
  const { t } = useTranslation()
  const {
    nativeControlsReservePx,
    handleMinimizeWindow,
    handleToggleMaximizeWindow,
    handleCloseWindow,
  } = useWindowControls(true)

  useEffect(() => {
    if (isLoaded) onReady()
  }, [isLoaded, onReady])

  return (
    <div
      className="app"
      data-tab-width-mode={config.tab_width_mode}
      data-parity="workspace.titlebar.app"
      style={
        {
          "--tab-standard-width": `${config.tab_standard_width}px`,
        } as CSSProperties
      }
    >
      <div className="title-bar" data-parity="workspace.titlebar">
        <TitleBarTabs
          tabs={workspaceTitleBarTabs}
          groups={workspaceTitleBarGroups}
          activeTabId={workspaceTitleBarActiveTabId}
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
        <div className="drag-space" data-tauri-drag-region></div>
        <TitleBarActions
          nativeControlsReservePx={nativeControlsReservePx}
          runningTunnelCount={0}
          settingsTabTitle={t("settings.title")}
          tunnelsButtonLabel={t("tunnels.title")}
          isLinux={false}
          onTunnelsClick={() => {}}
          onSettingsClick={() => {}}
          onMinimizeWindow={handleMinimizeWindow}
          onToggleMaximizeWindow={handleToggleMaximizeWindow}
          onCloseWindow={handleCloseWindow}
        />
      </div>
      <div className="content-area" data-parity="workspace.titlebar.content" />
    </div>
  )
}
