import React from "react"

import { filterProfiles } from "@/components/ProfilesPanel/profileListing"
import { buildGroupTree, type GroupTree } from "@/lib/profileGroups"
import type { ProfileGroup, SavedProfile } from "@/types/tab"

/**
 * The search on the groups page. It finds what the connections panel finds,
 * but keeps the tree, holding only the matches, so every group still shows
 * and takes a drop of what was found.
 */
export function useProfileGroupSearch(
  profileGroups: readonly ProfileGroup[],
  profiles: SavedProfile[]
) {
  const [query, setQuery] = React.useState("")
  const isFiltering = query.trim().length > 0
  const matchedProfiles = React.useMemo(
    () => (isFiltering ? filterProfiles(profiles, query) : profiles),
    [isFiltering, profiles, query]
  )
  /** The tree of the matches while searching, or null to show the whole tree. */
  const searchTree: GroupTree | null = React.useMemo(
    () => (isFiltering ? buildGroupTree(profileGroups, matchedProfiles) : null),
    [isFiltering, matchedProfiles, profileGroups]
  )

  return { query, setQuery, isFiltering, matchedProfiles, searchTree }
}
