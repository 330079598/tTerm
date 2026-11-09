import { createInstance } from "i18next"
import { getTabCloseMenuActions } from "@/lib/tabClosing"
import { getReopenClosedTabMenuAction } from "@/lib/tabMenuActions"
import {
  DEFAULT_KEYMAP_CONFIG,
  resolveEffectiveKeymap,
  getShortcutLabel,
} from "@/lib/keymap/keymap"
import en from "@/i18n/locales/en.json"
import type { TabContextMenuAction } from "@/types/tab"

/** The unsplit page-tab branch in TabBar/TabItem.tsx, with the first of two tabs. */
export function pageTabMenu() {
  const translation = createInstance()
  void translation.init({ resources: { en: { translation: en } }, lng: "en", initAsync: false })
  const t = translation.t
  const actions: TabContextMenuAction[] = [
    { label: t("contextMenu.newTab"), action: "new", icon: "plus", shortcut: "workspace.newTab" },
    getReopenClosedTabMenuAction(t),
    { separator: true, label: "", action: "" },
    ...getTabCloseMenuActions(["settings", "logs"], "settings", {
      closeTab: t("contextMenu.closeTab"),
      closeOtherTabs: t("contextMenu.closeOtherTabs"),
      closeTabsToLeft: t("contextMenu.closeTabsToLeft"),
      closeTabsToRight: t("contextMenu.closeTabsToRight"),
    }),
  ]
  return actions
}

/** Generated GPUI fixture chords come from the same default resolver as KeymapProvider. */
export function contextMenuFixture() {
  const actions = pageTabMenu()
  const labels = (isMac: boolean) =>
    actions.map((action) =>
      action.shortcut
        ? (getShortcutLabel(
            resolveEffectiveKeymap(DEFAULT_KEYMAP_CONFIG, isMac),
            action.shortcut,
            isMac
          ) ?? null)
        : null
    )
  return {
    commands: { load_config: { $pending: true }, load_app_state: { $pending: true } },
    actions,
    shortcuts: { macos: labels(true), windows: labels(false) },
  }
}
