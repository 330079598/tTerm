import type { useTranslation } from "react-i18next"

import type { KeymapActionId } from "@/lib/keymap/actions"
import { hasTabIcon } from "@/lib/tabIcons"
import { getTabLogMenuActions } from "@/lib/terminalLogRecording"
import { isPageTab, isSessionTab, type Tab, type TabContextMenuAction } from "@/types/tab"

const SPLITS: {
  key: string
  label: string
  action: string
  shortcut?: KeymapActionId
}[] = [
  {
    key: "splitRight",
    label: "Split Right",
    action: "split-right",
    shortcut: "workspace.splitRight",
  },
  { key: "splitLeft", label: "Split Left", action: "split-left" },
  { key: "splitDown", label: "Split Down", action: "split-down", shortcut: "workspace.splitBelow" },
  { key: "splitAbove", label: "Split Above", action: "split-above" },
]

/** The menu items that split a terminal tab's pane, none for other tabs. */
export function getTabSplitMenuActions(
  tab: Pick<Tab, "type">,
  t: ReturnType<typeof useTranslation>["t"]
): TabContextMenuAction[] {
  if (!isSessionTab(tab)) return []
  return SPLITS.map(({ key, label, action, shortcut }) => ({
    label: t(`contextMenu.${key}`, { defaultValue: label }),
    action,
    icon: action,
    shortcut,
  }))
}

/** The item that renames a tab; a terminal tab's also picks its icon. */
export function getTabRenameMenuAction(
  tab: Pick<Tab, "type">,
  t: ReturnType<typeof useTranslation>["t"]
): TabContextMenuAction {
  const label = hasTabIcon(tab) ? t("contextMenu.editTab") : t("contextMenu.renameTab")
  return { label, action: "rename", icon: "edit", shortcut: "tabs.renameActive" }
}

/** The item that pins or unpins an SSH tab's connection header, none for other tabs. */
export function getTabPinMenuAction(
  tab: Pick<Tab, "type" | "connectionHeaderPinned">,
  t: ReturnType<typeof useTranslation>["t"]
): TabContextMenuAction[] {
  if (tab.type !== "ssh") return []
  return tab.connectionHeaderPinned === false
    ? [{ label: t("contextMenu.pinConnectionHeader"), action: "pin-header", icon: "pin" }]
    : [{ label: t("contextMenu.unpinConnectionHeader"), action: "unpin-header", icon: "pin-off" }]
}

/**
 * The item that edits the saved connection a session tab came from, or
 * saves an SSH or local tab that has none as one. A serial tab only edits
 * the profile it came from.
 */
export function getTabConnectionMenuAction(
  tab: Pick<Tab, "type" | "connection">,
  t: ReturnType<typeof useTranslation>["t"]
): TabContextMenuAction[] {
  const label = tabConnectionActionLabel(tab)
  if (!label) return []
  return [
    { label: t(label), action: "edit-connection", icon: "edit", shortcut: "tabs.saveConnection" },
  ]
}

/**
 * The label key of {@link getTabConnectionMenuAction}'s item for `tab`, or
 * null when it has none, so the shortcut acts where the menu would.
 */
export function tabConnectionActionLabel(tab: Pick<Tab, "type" | "connection">): string | null {
  if (!isSessionTab(tab)) return null
  if (tab.connection?.profileId) return "contextMenu.editConnection"
  if (tab.type === "ssh") return "contextMenu.saveConnection"
  if (tab.type === "terminal") return "contextMenu.saveLocalConnection"
  return null
}

/** The item that opens the tab closed last again. */
export function getReopenClosedTabMenuAction(
  t: ReturnType<typeof useTranslation>["t"]
): TabContextMenuAction {
  return {
    label: t("contextMenu.reopenClosedTab"),
    action: "reopen-closed",
    icon: "rotate-ccw",
    shortcut: "tabs.reopenClosed",
  }
}

/** The item that puts a tab of the bar away, to open it again from the collapsed tabs. */
export function getCollapseTabMenuAction(
  t: ReturnType<typeof useTranslation>["t"]
): TabContextMenuAction {
  return { label: t("contextMenu.collapseTab"), action: "collapse-tab", icon: "collapse-tab" }
}

/** The item that undoes a tab's split, giving each of its panes a tab of its own. */
export function getUngroupMenuAction(
  t: ReturnType<typeof useTranslation>["t"]
): TabContextMenuAction {
  return {
    label: t("workspace.unsplit"),
    action: "ungroup",
    icon: "unsplit",
    shortcut: "workspace.restoreSingleGroup",
  }
}

/** The item that opens the layout templates for the tab holding this pane. */
export function getLayoutTemplatesMenuAction(
  t: ReturnType<typeof useTranslation>["t"]
): TabContextMenuAction {
  return {
    label: t("layoutTemplates.menu"),
    action: "layout-templates",
    icon: "layout",
    shortcut: "workspace.chooseLayout",
  }
}

/** The context menu of a pane's header, in a tab of the bar split into panes. */
export function getPaneMenuActions(
  tab: Tab,
  pane: { isZoomed: boolean },
  t: ReturnType<typeof useTranslation>["t"]
): TabContextMenuAction[] {
  const layoutActions: TabContextMenuAction[] = [
    { separator: true, label: "", action: "" },
    {
      label: pane.isZoomed ? t("workspace.unzoomPane") : t("workspace.zoomPane"),
      action: "toggle-zoom",
      icon: pane.isZoomed ? "minimize" : "maximize",
      shortcut: "workspace.togglePaneZoom",
    },
    {
      label: t("workspace.movePaneToNewTab"),
      action: "move-to-new-tab",
      icon: "move-out",
      shortcut: "workspace.movePaneToNewTab",
    },
    getUngroupMenuAction(t),
    getLayoutTemplatesMenuAction(t),
  ]
  const closeAction: TabContextMenuAction[] = [
    { separator: true, label: "", action: "" },
    { label: t("workspace.closePane"), action: "close", icon: "x", shortcut: "tabs.closeActive" },
  ]

  if (isPageTab(tab)) {
    return [...layoutActions.slice(1), ...closeAction]
  }

  return [
    {
      label: t("contextMenu.duplicateTab"),
      action: "duplicate",
      icon: "copy",
      shortcut: "tabs.duplicateActive",
    },
    ...getTabSplitMenuActions(tab, t),
    getTabRenameMenuAction(tab, t),
    ...getTabConnectionMenuAction(tab, t),
    ...getTabPinMenuAction(tab, t),
    ...getTabLogMenuActions(tab, t),
    ...layoutActions,
    ...closeAction,
  ]
}
