import type { useTranslation } from "react-i18next"

import type { Tab, TabContextMenuAction } from "@/types/tab"

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
