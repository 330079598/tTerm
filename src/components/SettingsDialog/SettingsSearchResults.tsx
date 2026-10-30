import React, { useEffect, useRef } from "react"
import { ChevronRight, Search, SearchX } from "lucide-react"
import { useTranslation } from "react-i18next"

import {
  SETTINGS_SEARCH_RESULTS_ID,
  settingsSearchResultId,
} from "@/components/SettingsDialog/settingsSearch"
import { SETTINGS_TABS } from "@/components/SettingsDialog/settingsTabs"
import type { SettingsSearch } from "@/components/SettingsDialog/useSettingsSearch"
import { cn } from "@/lib/utils"

interface SettingsSearchProps {
  search: SettingsSearch
}

/** The settings that match the search, shown in place of the current page. */
export const SettingsSearchResults: React.FC<SettingsSearchProps> = ({ search }) => {
  const { t } = useTranslation()
  const listRef = useRef<HTMLDivElement>(null)
  const { results, activeIndex } = search

  useEffect(() => {
    listRef.current
      ?.querySelector(`#${settingsSearchResultId(activeIndex)}`)
      ?.scrollIntoView?.({ block: "nearest" })
  }, [activeIndex])

  if (results.length === 0) {
    return (
      <div className="text-muted-foreground flex min-w-0 flex-1 flex-col items-center justify-center gap-2 p-6 text-sm">
        <SearchX size={24} aria-hidden="true" />
        {t("settings.search.empty", { query: search.query.trim() })}
      </div>
    )
  }

  return (
    <div ref={listRef} className="min-w-0 flex-1 overflow-y-auto p-4 sm:p-6">
      <div className="text-muted-foreground mb-2 px-2 text-xs">
        {t("settings.search.count", { count: results.length })}
      </div>
      <div id={SETTINGS_SEARCH_RESULTS_ID} role="listbox" aria-label={t("settings.search.label")}>
        {results.map((result, index) => {
          const Icon = SETTINGS_TABS.find((tab) => tab.value === result.tab)?.icon ?? Search
          const isActive = index === activeIndex
          return (
            <div
              key={`${result.tab}:${result.title}:${index}`}
              id={settingsSearchResultId(index)}
              role="option"
              aria-selected={isActive}
              // The field keeps the focus, so a click must not take it away.
              onMouseDown={(event) => event.preventDefault()}
              onMouseMove={() => !isActive && search.setActiveIndex(index)}
              onClick={() => search.openResult(result)}
              className={cn(
                "flex cursor-pointer items-start gap-3 rounded-md px-3 py-2.5",
                // Muted rather than the accent, which some themes make a
                // saturated color that outshines the text on it.
                isActive && "bg-muted"
              )}
            >
              <Icon size={16} className="text-muted-foreground mt-0.5 shrink-0" aria-hidden />
              <div className="min-w-0 flex-1">
                <div className="flex min-w-0 items-center gap-1 text-sm font-medium">
                  {!result.isPage && (
                    <>
                      <span className="text-muted-foreground shrink-0 font-normal">
                        {result.tabLabel}
                      </span>
                      <ChevronRight
                        size={12}
                        className="text-muted-foreground shrink-0"
                        aria-hidden
                      />
                    </>
                  )}
                  <span className="truncate">{result.title}</span>
                </div>
                {result.description && (
                  <div className="text-muted-foreground mt-0.5 line-clamp-2 text-xs leading-5">
                    {result.description}
                  </div>
                )}
              </div>
            </div>
          )
        })}
      </div>
    </div>
  )
}
