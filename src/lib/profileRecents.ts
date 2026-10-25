/** When a saved connection last reached a shell or opened its port. */
export interface RecentProfile {
  profileId: string
  lastUsedAt: number
}

export const PROFILE_RECENTS_STORAGE_KEY = "tterm.profile-recents.v1"
/**
 * More than the launcher shows, since a search also puts the saved
 * connections used lately first.
 */
export const PROFILE_RECENTS_LIMIT = 50

/** Moves the profile to the front, newest first. */
export function addRecentProfile(
  recents: readonly RecentProfile[],
  profileId: string,
  usedAt: number
): RecentProfile[] {
  return [
    { profileId, lastUsedAt: usedAt },
    ...recents.filter((recent) => recent.profileId !== profileId),
  ].slice(0, PROFILE_RECENTS_LIMIT)
}

export function removeRecentProfile(
  recents: readonly RecentProfile[],
  profileId: string
): RecentProfile[] {
  return recents.filter((recent) => recent.profileId !== profileId)
}

/** When each profile was last used, for sorting. */
export function profileLastUsed(recents: readonly RecentProfile[]): Map<string, number> {
  return new Map(recents.map((recent) => [recent.profileId, recent.lastUsedAt]))
}

export function loadRecentProfiles(storage: Pick<Storage, "getItem">): RecentProfile[] {
  try {
    const value = storage.getItem(PROFILE_RECENTS_STORAGE_KEY)
    if (!value) return []
    const parsed: unknown = JSON.parse(value)
    if (!Array.isArray(parsed)) return []
    return parsed
      .filter(
        (item): item is RecentProfile =>
          item !== null &&
          typeof item === "object" &&
          typeof item.profileId === "string" &&
          typeof item.lastUsedAt === "number"
      )
      .slice(0, PROFILE_RECENTS_LIMIT)
  } catch {
    return []
  }
}

export function saveRecentProfiles(
  storage: Pick<Storage, "setItem">,
  recents: readonly RecentProfile[]
) {
  try {
    storage.setItem(PROFILE_RECENTS_STORAGE_KEY, JSON.stringify(recents))
  } catch {
    // Recents are a convenience; a full or blocked storage just forgets them.
  }
}
