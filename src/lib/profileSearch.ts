import type { SavedProfile } from "@/types/tab"

/**
 * The fields a search for saved connections looks in, so that the
 * connections panel and the new tab launcher find the same connections.
 */
export function profileSearchFields(profile: SavedProfile): Array<string | undefined> {
  return [profile.name, profile.group, profile.host, profile.username, profile.serial?.port]
}

const NAME_WORD_SEPARATORS = /[\s\-_.]+/

/**
 * How well a profile that matches a search fits it, higher first: 4 for the
 * name itself, 3 for the start of the name or of a word in it, 2 for every
 * word in the name or the start of the host, 1 for a match elsewhere. An
 * empty search scores 0.
 */
export function profileMatchScore(profile: SavedProfile, query: string): number {
  const phrase = query.trim().toLowerCase().replace(/\s+/g, " ")
  if (!phrase) return 0
  const name = profile.name.toLowerCase()
  if (name === phrase) return 4
  if (
    name.startsWith(phrase) ||
    name.split(NAME_WORD_SEPARATORS).some((word) => word.startsWith(phrase))
  ) {
    return 3
  }
  if (
    phrase.split(" ").every((word) => name.includes(word)) ||
    (profile.host ?? "").toLowerCase().startsWith(phrase)
  ) {
    return 2
  }
  return 1
}

/**
 * Profiles that match a search, best fit first; among equal fits the one
 * used last first, then the saved order.
 */
export function rankProfiles(
  profiles: readonly SavedProfile[],
  query: string,
  lastUsed: ReadonlyMap<string, number> = new Map()
): SavedProfile[] {
  const scores = new Map(profiles.map((profile) => [profile.id, profileMatchScore(profile, query)]))
  // Array.prototype.sort is stable, so equal profiles keep the saved order.
  return [...profiles].sort(
    (left, right) =>
      (scores.get(right.id) ?? 0) - (scores.get(left.id) ?? 0) ||
      (lastUsed.get(right.id) ?? 0) - (lastUsed.get(left.id) ?? 0)
  )
}
