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

/**
 * Profiles by group, groups by name with the ungrouped ones last, each
 * group in the saved order.
 */
export function groupProfiles(profiles: readonly SavedProfile[]): [string, SavedProfile[]][] {
  const groups = new Map<string, SavedProfile[]>()

  for (const profile of profiles) {
    const key = profile.group === "" ? UNGROUPED_KEY : profile.group
    const existingItems = groups.get(key)
    if (existingItems) {
      existingItems.push(profile)
    } else {
      groups.set(key, [profile])
    }
  }

  return Array.from(groups.entries()).sort(([left], [right]) => {
    if (left === UNGROUPED_KEY) return 1
    if (right === UNGROUPED_KEY) return -1
    return left.localeCompare(right)
  })
}
