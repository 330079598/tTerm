import { useTranslation } from "react-i18next"
import { Search, X } from "lucide-react"

import type { TabBarProps } from "@/components/TabBar"
import type { useTabOverflowMenu } from "@/components/TabBar/useTabOverflowMenu"
import { TabAttentionDot } from "@/components/TabAttentionDot"
import { getOverflowConnectionMeta } from "@/components/TabBar/overflowConnectionMeta"
import { TabIcon } from "@/components/TabIcon"

type TabOverflowPanelProps = Pick<
  ReturnType<typeof useTabOverflowMenu>,
  | "overflowPanelRef"
  | "overflowPanelStyle"
  | "searchInputRef"
  | "searchQuery"
  | "setSearchQuery"
  | "handleSearchKeyDown"
  | "filteredTabs"
  | "handleSelectTab"
> &
  Pick<TabBarProps, "tabs" | "activeTabId">

/** The overflow menu's panel: a search box and the tabs that match it. */
export function TabOverflowPanel({
  overflowPanelRef,
  overflowPanelStyle,
  searchInputRef,
  searchQuery,
  setSearchQuery,
  handleSearchKeyDown,
  filteredTabs,
  tabs,
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
        {filteredTabs.map((tab) => {
          const tabIndex = tabs.findIndex((currentTab) => currentTab.id === tab.id)
          const connectionMeta = getOverflowConnectionMeta(tab)

          return (
            <button
              key={tab.id}
              type="button"
              className={`tab-overflow-item ${tab.id === activeTabId ? "active" : ""}`}
              onClick={() => handleSelectTab(tab.id)}
            >
              <span className="tab-overflow-number">{tabIndex + 1}</span>
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
        })}

        {filteredTabs.length === 0 && (
          <div className="tab-overflow-empty">
            {t("tabs.noMatchingTabs", { defaultValue: "No matching tabs" })}
          </div>
        )}
      </div>
    </div>
  )
}
