import { useTranslation } from "react-i18next"
import { Minus, Settings, Square, Waypoints, X } from "lucide-react"

import { keepFocusOnMouseDown } from "@/lib/terminalFocus"

interface TitleBarActionsProps {
  nativeControlsReservePx: number
  runningTunnelCount: number
  settingsTabTitle: string
  tunnelsButtonLabel: string
  isLinux: boolean
  onTunnelsClick: () => void
  onSettingsClick: () => void
  onMinimizeWindow: () => void
  onToggleMaximizeWindow: () => void
  onCloseWindow: () => void
}

/** The page and window buttons at the right end of the title bar. */
export function TitleBarActions({
  nativeControlsReservePx,
  runningTunnelCount,
  settingsTabTitle,
  tunnelsButtonLabel,
  isLinux,
  onTunnelsClick,
  onSettingsClick,
  onMinimizeWindow,
  onToggleMaximizeWindow,
  onCloseWindow,
}: TitleBarActionsProps) {
  const { t } = useTranslation()

  return (
    <div
      className="title-bar-right"
      data-parity="workspace.titlebar.actions"
      style={{ paddingRight: `${nativeControlsReservePx}px` }}
    >
      <button
        className="tab-action settings-button tunnels-trigger"
        data-parity="workspace.titlebar.tunnels"
        onMouseDown={keepFocusOnMouseDown}
        onClick={onTunnelsClick}
        aria-label={tunnelsButtonLabel}
        title={tunnelsButtonLabel}
      >
        <Waypoints size={16} />
        {runningTunnelCount > 0 && (
          <span className="tunnels-running-badge">
            {runningTunnelCount > 99 ? "99+" : runningTunnelCount}
          </span>
        )}
      </button>
      <button
        className="tab-action settings-button"
        data-parity="workspace.titlebar.settings"
        onMouseDown={keepFocusOnMouseDown}
        onClick={onSettingsClick}
        aria-label={settingsTabTitle}
      >
        <Settings size={16} />
      </button>
      {isLinux && (
        <div
          className="window-controls"
          aria-label={t("window.controls", { defaultValue: "Window controls" })}
        >
          <button
            className="window-control-button"
            data-parity="workspace.titlebar.minimize"
            onClick={onMinimizeWindow}
            aria-label={t("window.minimize", { defaultValue: "Minimize" })}
          >
            <Minus size={16} />
          </button>
          <button
            className="window-control-button"
            data-parity="workspace.titlebar.maximize"
            onClick={onToggleMaximizeWindow}
            aria-label={t("window.maximize", { defaultValue: "Maximize" })}
          >
            <Square size={13} />
          </button>
          <button
            className="window-control-button close"
            data-parity="workspace.titlebar.close"
            onClick={onCloseWindow}
            aria-label={t("window.close", { defaultValue: "Close" })}
          >
            <X size={16} />
          </button>
        </div>
      )}
    </div>
  )
}
