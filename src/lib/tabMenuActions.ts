import type { useTranslation } from "react-i18next"

import { getTabCloseMenuActions } from "@/lib/tabClosing"
import { getTabLogMenuActions } from "@/lib/terminalLogRecording"
import { isPageTab, type Tab, type TabContextMenuAction } from "@/types/tab"

const SPLITS = [
  { key: "splitRight", label: "Split Right", action: "split-right" },
  { key: "splitLeft", label: "Split Left", action: "split-left" },
  { key: "splitDown", label: "Split Down", action: "split-down" },
  { key: "splitAbove", label: "Split Above", action: "split-above" },
] as const

/** The menu items that split a terminal tab's pane, none for other tabs. */
export function getTabSplitMenuActions(
  tab: Pick<Tab, "type">,
  t: ReturnType<typeof useTranslation>["t"]
): TabContextMenuAction[] {
  if (tab.type !== "terminal" && tab.type !== "ssh") return []
  return SPLITS.map(({ key, label, action }) => ({
    label: t(`contextMenu.${key}`, { defaultValue: label }),
    action,
    icon: action,
  }))
}

/** The item that renames a tab; a local tab's also picks its icon. */
export function getTabRenameMenuAction(
  tab: Pick<Tab, "type">,
  t: ReturnType<typeof useTranslation>["t"]
): TabContextMenuAction {
  const label = tab.type === "terminal" ? t("contextMenu.editTab") : t("contextMenu.renameTab")
  return { label, action: "rename", icon: "edit" }
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

/** The context menu of a tab in a split group's header. */
export function getWorkspaceTabMenuActions(
  tab: Tab,
  tabIds: string[],
  t: ReturnType<typeof useTranslation>["t"]
): TabContextMenuAction[] {
  const commonStart: TabContextMenuAction[] = [
    { label: t("contextMenu.newTab"), action: "new", icon: "plus" },
  ]
  const commonEnd: TabContextMenuAction[] = [
    { separator: true, label: "", action: "" },
    ...getTabCloseMenuActions(tabIds, tab.id, {
      closeTab: t("contextMenu.closeTab"),
      closeOtherTabs: t("contextMenu.closeOtherTabs"),
      closeTabsToLeft: t("contextMenu.closeTabsToLeft"),
      closeTabsToRight: t("contextMenu.closeTabsToRight"),
    }),
  ]

  if (isPageTab(tab)) {
    return [...commonStart, ...commonEnd]
  }

  const editConnectionAction: TabContextMenuAction | null =
    tab.type === "ssh" && tab.connection?.profileId
      ? { label: t("contextMenu.editConnection"), action: "edit-connection", icon: "edit" }
      : null
  const logActions = getTabLogMenuActions(tab, t)

  return [
    ...commonStart,
    { label: t("contextMenu.duplicateTab"), action: "duplicate", icon: "copy" },
    ...getTabSplitMenuActions(tab, t),
    getTabRenameMenuAction(tab, t),
    ...(editConnectionAction ? [editConnectionAction] : []),
    ...getTabPinMenuAction(tab, t),
    ...logActions,
    ...commonEnd,
  ]
}
