import type { SavedProfile } from "@/types/tab"

/**
 * The fields a search for saved connections looks in, so that the
 * connections panel and the new tab launcher find the same connections.
 */
export function profileSearchFields(profile: SavedProfile): Array<string | undefined> {
  return [profile.name, profile.group, profile.host, profile.username, profile.serial?.port]
}
