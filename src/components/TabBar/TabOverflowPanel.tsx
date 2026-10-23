import React from "react"
import { useTranslation } from "react-i18next"
import { Search, X } from "lucide-react"

import type { TabBarProps } from "@/components/TabBar"
import type { useTabOverflowMenu } from "@/components/TabBar/useTabOverflowMenu"
import { TabAttentionDot } from "@/components/TabAttentionDot"
import { getOverflowConnectionMeta } from "@/components/TabBar/overflowConnectionMeta"
import { TabIcon } from "@/components/TabIcon"
import type { Tab } from "@/types/tab"

type TabOverflowPanelProps = Pick<
  ReturnType<typeof useTabOverflowMenu>,
  | "overflowPanelRef"
  | "overflowPanelStyle"
  | "searchInputRef"
  | "searchQuery"
  | "setSearchQuery"
  | "handleSearchKeyDown"
  | "entries"
  | "handleSelectTab"
> &
  Pick<TabBarProps, "activeTabId">

/** One row of the menu: a tab of the bar, or a pane of a split one. */
function OverflowRow({
  tab,
  number,
  isActive,
  isPane,
  onSelect,
}: {
  tab: Tab
  number?: number
  isActive: boolean
  isPane?: boolean
  onSelect: (id: string) => void
}) {
  const connectionMeta = getOverflowConnectionMeta(tab)
  return (
    <button
      type="button"
      className={`tab-overflow-item ${isPane ? "pane" : ""} ${isActive ? "active" : ""}`}
      onClick={() => onSelect(tab.id)}
    >
      <span className="tab-overflow-number">{number}</span>
      <TabIcon tab={tab} className="tab-icon" />
      <TabAttentionDot tabId={tab.id} />
      <span className="tab-overflow-title">{tab.title}</span>
      {connectionMeta.primary && (
        <span className="tab-overflow-meta">
          <span className="tab-overflow-meta-primary">{connectionMeta.primary}</span>
          {connectionMeta.secondary && (
            <span className="tab-overflow-meta-secondary">{connectionMeta.secondary}</span>
          )}
        </span>
      )}
    </button>
  )
}

/** The overflow menu's panel: a search box and the tabs, and panes of split tabs, that match it. */
export function TabOverflowPanel({
  overflowPanelRef,
  overflowPanelStyle,
  searchInputRef,
  searchQuery,
  setSearchQuery,
  handleSearchKeyDown,
  entries,
  activeTabId,
  handleSelectTab,
}: TabOverflowPanelProps) {
  const { t } = useTranslation()

  return (
    <div
      ref={overflowPanelRef}
      className="tab-overflow-panel"
      role="dialog"
      aria-label={t("tabs.searchTabs", { defaultValue: "Search tabs" })}
      style={overflowPanelStyle ?? undefined}
    >
      <div className="tab-search-box">
        <Search size={14} />
        <input
          ref={searchInputRef}
          className="tab-search-input"
          value={searchQuery}
          placeholder={t("tabs.searchTabs", { defaultValue: "Search tabs" })}
          onChange={(event) => setSearchQuery(event.target.value)}
          onKeyDown={handleSearchKeyDown}
        />
        {searchQuery && (
          <button
            type="button"
            className="tab-search-clear"
            aria-label={t("tabs.clearSearch", { defaultValue: "Clear search" })}
            onClick={() => {
              setSearchQuery("")
              searchInputRef.current?.focus()
            }}
          >
            <X size={13} />
          </button>
        )}
      </div>

      <div className="tab-overflow-results">
        {entries.map(({ tab, number, panes }) => (
          <React.Fragment key={tab.id}>
            <OverflowRow
              tab={tab}
              number={number}
              isActive={tab.id === activeTabId && !panes.some(({ id }) => id === activeTabId)}
              onSelect={handleSelectTab}
            />
            {panes.map((pane) => (
              <OverflowRow
                key={pane.id}
                tab={pane}
                isActive={pane.id === activeTabId}
                isPane
                onSelect={handleSelectTab}
              />
            ))}
          </React.Fragment>
        ))}

        {entries.length === 0 && (
          <div className="tab-overflow-empty">
            {t("tabs.noMatchingTabs", { defaultValue: "No matching tabs" })}
          </div>
        )}
      </div>
    </div>
  )
}
