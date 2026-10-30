import React from "react"
import { Search } from "lucide-react"
import { useTranslation } from "react-i18next"

import {
  SETTINGS_SEARCH_RESULTS_ID,
  settingsSearchResultId,
} from "@/components/SettingsDialog/settingsSearch"
import type { SettingsSearch } from "@/components/SettingsDialog/useSettingsSearch"
import { Input } from "@/components/ui/input"

interface SettingsSearchProps {
  search: SettingsSearch
}

/** The field that searches every settings page. */
export const SettingsSearchField: React.FC<SettingsSearchProps> = ({ search }) => {
  const { t } = useTranslation()
  const hasResults = search.query.trim() !== "" && search.results.length > 0

  return (
    <div className="relative w-full sm:w-64">
      <Search className="text-muted-foreground pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2" />
      <Input
        type="search"
        role="combobox"
        aria-expanded={hasResults}
        aria-controls={SETTINGS_SEARCH_RESULTS_ID}
        aria-activedescendant={hasResults ? settingsSearchResultId(search.activeIndex) : undefined}
        aria-label={t("settings.search.label")}
        placeholder={t("settings.search.placeholder")}
        value={search.query}
        onChange={(event) => search.setQuery(event.target.value)}
        onKeyDown={search.handleKeyDown}
        className="h-8 pl-8"
      />
    </div>
  )
}
