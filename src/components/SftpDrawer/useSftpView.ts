import { useCallback, useMemo, useState } from "react"

import {
  createSftpSearchMatcher,
  DEFAULT_SFTP_SEARCH_OPTIONS,
  type SftpSearchOptions,
} from "@/components/SftpDrawer/sftpSearch"
import type { SftpDirectoryListing } from "@/components/SftpDrawer/types"
import {
  applySftpView,
  loadSftpViewPreferences,
  nextSftpSort,
  saveSftpViewPreferences,
  type SftpSortColumn,
  type SftpViewPreferences,
} from "@/components/SftpDrawer/sftpView"

/**
 * How the listing is shown: the search options, the sort order and whether hidden
 * entries appear. The last two are remembered across sessions.
 */
export function useSftpView(listing: SftpDirectoryListing | null, searchQuery: string) {
  const [searchOptions, setSearchOptions] = useState<SftpSearchOptions>(DEFAULT_SFTP_SEARCH_OPTIONS)
  const [viewPreferences, setViewPreferences] = useState(loadSftpViewPreferences)

  const searchMatcher = useMemo(
    () => createSftpSearchMatcher(searchQuery, searchOptions),
    [searchOptions, searchQuery]
  )

  const updateViewPreferences = useCallback(
    (update: (current: SftpViewPreferences) => SftpViewPreferences) => {
      setViewPreferences((current) => {
        const next = update(current)
        saveSftpViewPreferences(next)
        return next
      })
    },
    []
  )

  const handleSortColumn = useCallback(
    (column: SftpSortColumn) => {
      updateViewPreferences((current) => ({
        ...current,
        sort: nextSftpSort(current.sort, column),
      }))
    },
    [updateViewPreferences]
  )

  const toggleShowHidden = useCallback(() => {
    updateViewPreferences((current) => ({ ...current, showHidden: !current.showHidden }))
  }, [updateViewPreferences])

  // The listing as shown: hidden entries dropped, rows in sort order. Range
  // selection, select-all and the table all follow this order.
  const viewListing = useMemo(
    () =>
      listing ? { ...listing, entries: applySftpView(listing.entries, viewPreferences) } : listing,
    [listing, viewPreferences]
  )

  const filteredListing = useMemo(() => {
    if (!viewListing || !searchMatcher.hasQuery) {
      return viewListing
    }

    return {
      ...viewListing,
      entries: viewListing.entries.filter(searchMatcher.matches),
    }
  }, [viewListing, searchMatcher])

  const toggleSearchOption = useCallback((option: keyof SftpSearchOptions) => {
    setSearchOptions((current) => ({
      ...current,
      [option]: !current[option],
    }))
  }, [])

  return {
    searchOptions,
    toggleSearchOption,
    searchMatcher,
    viewPreferences,
    handleSortColumn,
    toggleShowHidden,
    viewListing,
    filteredListing,
  }
}
