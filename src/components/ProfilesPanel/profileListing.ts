import { profileLastUsed, type RecentProfile } from "@/lib/profileRecents"
import { profileSearchFields } from "@/lib/profileSearch"
import { matchesQuery } from "@/lib/quickConnect"
import type { SavedProfile } from "@/types/tab"

/** The key of the group that holds the profiles in no group. */
export const UNGROUPED_KEY = "__ungrouped__"

/** How the panel lists profiles: by group, or in one list by when they were last used. */
export type ProfileSort = "default" | "recent"

export const PROFILE_SORT_STORAGE_KEY = "tterm.profiles.sort.v1"

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

/**
 * The profiles in one list, the one used last first. Profiles never used
 * follow in the saved order.
 */
export function sortProfilesByRecent(
  profiles: readonly SavedProfile[],
  recents: readonly RecentProfile[]
): SavedProfile[] {
  const lastUsed = profileLastUsed(recents)
  // Array.prototype.sort is stable, so equal times keep the saved order.
  return [...profiles].sort(
    (left, right) => (lastUsed.get(right.id) ?? 0) - (lastUsed.get(left.id) ?? 0)
  )
}

export function loadProfileSort(storage: Pick<Storage, "getItem">): ProfileSort {
  try {
    return storage.getItem(PROFILE_SORT_STORAGE_KEY) === "recent" ? "recent" : "default"
  } catch {
    return "default"
  }
}

export function saveProfileSort(storage: Pick<Storage, "setItem">, sort: ProfileSort) {
  try {
    storage.setItem(PROFILE_SORT_STORAGE_KEY, sort)
  } catch {
    // The order is a convenience; a blocked storage just starts by group again.
  }
}
