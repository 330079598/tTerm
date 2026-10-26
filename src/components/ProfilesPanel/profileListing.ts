import { profileSearchFields } from "@/lib/profileSearch"
import { matchesQuery } from "@/lib/quickConnect"
import type { SavedProfile } from "@/types/tab"

/** The key of the group that holds the profiles in no group. */
export const UNGROUPED_KEY = "__ungrouped__"

/** The profiles that match a search; every profile for an empty one. */
export function filterProfiles(profiles: readonly SavedProfile[], query: string): SavedProfile[] {
  return profiles.filter((profile) =>
    matchesQuery(query, [...profileSearchFields(profile), profile.connection_type])
  )
}
