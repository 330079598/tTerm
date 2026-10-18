import React from "react"
import type { IDockviewHeaderActionsProps } from "dockview-react"
import {
  Columns2,
  Maximize2,
  Minimize2,
  PanelBottom,
  PanelLeft,
  PanelRight,
  PanelTop,
  PanelsTopLeft,
} from "lucide-react"
import { useTranslation } from "react-i18next"

import { useWorkspace } from "@/components/TTermApp/workspaceContext"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { useShortcutLabel } from "@/contexts/KeymapContext"
import { withShortcut } from "@/lib/keymap/keymap"
import { isSessionTab } from "@/types/tab"

export const WorkspaceHeaderActions: React.FC<IDockviewHeaderActionsProps> = ({
  activePanel,
  isGroupActive,
}) => {
  const { t } = useTranslation()
  const shortcutLabel = useShortcutLabel()
  const { collapseToSingleGroup, groupCount, maximizedGroupId, splitTab, tabs } = useWorkspace()
  const tab = activePanel ? tabs.find((candidate) => candidate.id === activePanel.id) : undefined
  const canSplit = isSessionTab(tab)
  const isMaximized = activePanel?.group.id === maximizedGroupId

  const action = (label: string, icon: React.ReactNode, onClick: () => void, disabled = false) => (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          className="workspace-header-action"
          aria-label={label}
          disabled={disabled}
          onClick={onClick}
        >
          {icon}
        </button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  )

  return (
    <div className={`workspace-header-actions ${isGroupActive ? "active" : ""}`}>
      {action(
        t("workspace.splitRight", { defaultValue: "Split Right" }),
        <PanelRight size={14} aria-hidden="true" />,
        () => activePanel && splitTab(activePanel.id, "right"),
        !canSplit
      )}
      {action(
        t("workspace.splitLeft", { defaultValue: "Split Left" }),
        <PanelLeft size={14} aria-hidden="true" />,
        () => activePanel && splitTab(activePanel.id, "left"),
        !canSplit
      )}
      {action(
        t("workspace.splitDown", { defaultValue: "Split Down" }),
        <PanelBottom size={14} aria-hidden="true" />,
        () => activePanel && splitTab(activePanel.id, "below"),
        !canSplit
      )}
      {action(
        t("workspace.splitAbove", { defaultValue: "Split Above" }),
        <PanelTop size={14} aria-hidden="true" />,
        () => activePanel && splitTab(activePanel.id, "above"),
        !canSplit
      )}
      {activePanel &&
        action(
          isMaximized
            ? t("workspace.restoreGroup", { defaultValue: "Restore Group" })
            : t("workspace.maximizeGroup", { defaultValue: "Maximize Group" }),
          isMaximized ? (
            <Minimize2 size={14} aria-hidden="true" />
          ) : (
            <Maximize2 size={14} aria-hidden="true" />
          ),
          () => (isMaximized ? activePanel.api.exitMaximized() : activePanel.api.maximize())
        )}
      {activePanel &&
        groupCount > 1 &&
        action(
          withShortcut(
            t("workspace.restoreSingleGroup", { defaultValue: "Restore Single Group" }),
            shortcutLabel("workspace.restoreSingleGroup")
          ),
          <PanelsTopLeft size={14} aria-hidden="true" />,
          () => collapseToSingleGroup(activePanel.id)
        )}
    </div>
  )
}

export const WorkspaceWatermark: React.FC = () => {
  const { t } = useTranslation()
  return (
    <div className="workspace-watermark">
      <Columns2 size={24} aria-hidden="true" />
      <span>{t("workspace.emptyGroup", { defaultValue: "Drag a tab here" })}</span>
    </div>
  )
}
