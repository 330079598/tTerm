import { useMemo, type ReactNode } from "react"
import { useTranslation } from "react-i18next"
import { BookMarked, Library, Plus } from "lucide-react"

import type { useTabContextMenu } from "@/hooks/useTabContextMenu"
import type { useWorkspaceSplits } from "@/components/TTermApp/useWorkspaceSplits"
import { TabBar } from "@/components/TabBar"
import { useShortcutLabel } from "@/contexts/KeymapContext"
import { getPaneIds, type TabGroup } from "@/lib/tabGroups"
import { withShortcut } from "@/lib/keymap/keymap"
import { keepFocusOnMouseDown } from "@/lib/terminalFocus"
import type { Tab } from "@/types/tab"

interface TitleBarTabsProps {
  tabs: Tab[]
  groups: TabGroup[]
  activeTabId: string | null
  setActiveTab: (id: string) => void
  handleCloseTab: (id: string) => Promise<void>
  handleNewTab: () => void
  moveTab: (fromIndex: number, toIndex: number) => void
  handleTabDragMove: ReturnType<typeof useWorkspaceSplits>["handleTabDragMove"]
  handleTabDrop: ReturnType<typeof useWorkspaceSplits>["handleTabDrop"]
  handleTabDragCancel: () => void
  handleTabContextMenu: ReturnType<typeof useTabContextMenu>["handleTabContextMenu"]
  setShowProfilesPanel: (show: boolean) => void
  onOpenCommandLibrary: () => void
  /** Extra title-bar tools, between the tab actions and the drag space. */
  children?: ReactNode
}

/** The tabs, the new-tab actions and the tools that share their strip. */
export function TitleBarTabs({
  tabs,
  groups,
  activeTabId,
  setActiveTab,
  handleCloseTab,
  handleNewTab,
  moveTab,
  handleTabDragMove,
  handleTabDrop,
  handleTabDragCancel,
  handleTabContextMenu,
  setShowProfilesPanel,
  onOpenCommandLibrary,
  children,
}: TitleBarTabsProps) {
  const { t } = useTranslation()
  const shortcutLabel = useShortcutLabel()
  const newTabLabel = t("tabs.newTab", { defaultValue: "New tab" })

  // The bar shows each group as the pane it had the focus in, with its other panes behind it.
  const { barTabs, tabPanes } = useMemo(() => {
    const byId = new Map(tabs.map((tab) => [tab.id, tab]))
    return {
      barTabs: groups.flatMap((group) => byId.get(group.activeTabId) ?? []),
      tabPanes: new Map(
        groups.map((group) => [
          group.activeTabId,
          {
            tabs: getPaneIds(group.root).flatMap((id) => byId.get(id) ?? []),
            isZoomed: group.zoomedTabId !== undefined,
          },
        ])
      ),
    }
  }, [groups, tabs])

  return (
    <div className="title-bar-left" data-parity="workspace.titlebar.tabs">
      <div className="tab-list-container">
        <TabBar
          tabs={barTabs}
          tabPanes={tabPanes}
          activeTabId={activeTabId}
          onTabClick={setActiveTab}
          onTabClose={handleCloseTab}
          onNewTab={handleNewTab}
          onTabMove={moveTab}
          onTabDragMove={handleTabDragMove}
          onTabDrop={handleTabDrop}
          onTabDragCancel={handleTabDragCancel}
          onContextMenu={handleTabContextMenu}
        />
        <div className="tab-add-button">
          <button
            className="tab-action"
            data-parity="workspace.titlebar.new-tab"
            onMouseDown={keepFocusOnMouseDown}
            onClick={handleNewTab}
            aria-label={newTabLabel}
            title={withShortcut(newTabLabel, shortcutLabel("workspace.newTab"))}
          >
            <Plus size={16} />
          </button>
          <button
            className="tab-action"
            data-parity="workspace.titlebar.profiles"
            onMouseDown={keepFocusOnMouseDown}
            onClick={() => setShowProfilesPanel(true)}
            aria-label={t("profiles.title", { defaultValue: "Profiles" })}
          >
            <BookMarked size={16} />
          </button>
          <button
            className="tab-action"
            data-parity="workspace.titlebar.command-library"
            onMouseDown={keepFocusOnMouseDown}
            onClick={onOpenCommandLibrary}
            aria-label={t("commandLibrary.title")}
            title={withShortcut(t("commandLibrary.title"), shortcutLabel("commandLibrary.open"))}
          >
            <Library size={16} />
          </button>
          {children}
        </div>
      </div>
    </div>
  )
}
