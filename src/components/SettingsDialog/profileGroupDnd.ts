import { normalizeSerialSettings } from "@/lib/serial"
import type { SavedProfile } from "@/types/tab"

export const UNGROUPED_KEY = "__ungrouped__"

/** A profile's group key: its group's id, or the ungrouped key. */
export const getGroupKey = (groupId: string | undefined) => groupId || UNGROUPED_KEY
export const getGroupIdFromKey = (key: string) => (key === UNGROUPED_KEY ? undefined : key)

export const profileDragId = (profileId: string) => `profile:${profileId}:drag`
export const profileDropId = (profileId: string) => `profile:${profileId}:drop`
/** A group's heading: dropping a profile or a group there files it in the group. */
export const groupDropId = (groupKey: string) => `profile-group:${groupKey}:drop`
/** The hint an empty group shows, which takes drops like its heading. */
export const groupBodyDropId = (groupKey: string) => `profile-group:${groupKey}:body`
export const groupDragId = (groupId: string) => `profile-group:${groupId}:drag`
/** Where a dragged group is dropped to move it to the top. */
export const ROOT_DROP_ID = "profile-group-root:drop"
export const groupListId = (groupKey: string) =>
  `profile-group-${encodeURIComponent(groupKey)}-list`

// Ids may hold colons (groups from older versions are keyed by their name),
// so the kind is read from the end.
const PROFILE_DND_ID = /^profile:(.+):(drag|drop)$/
const GROUP_DND_ID = /^profile-group:(.+):(drag|drop|body)$/

const parse = (pattern: RegExp, id?: string | number | null) =>
  id === null || id === undefined ? null : pattern.exec(String(id))

export const getProfileIdFromDndId = (id?: string | number | null): string | null =>
  parse(PROFILE_DND_ID, id)?.[1] ?? null

export const getProfileDropIdFromDndId = (id?: string | number | null): string | null => {
  const match = parse(PROFILE_DND_ID, id)
  return match?.[2] === "drop" ? match[1] : null
}

/** The group a drop target files into: a group's heading or empty hint. */
export const getGroupKeyFromDropId = (id?: string | number | null): string | null => {
  const match = parse(GROUP_DND_ID, id)
  return match && match[2] !== "drag" ? match[1] : null
}

export const getGroupIdFromDragId = (id?: string | number | null): string | null => {
  const match = parse(GROUP_DND_ID, id)
  return match?.[2] === "drag" ? match[1] : null
}

export const buildProfileSubtitle = (
  profile: SavedProfile,
  t: (key: string, options?: Record<string, unknown>) => string
) => {
  if (profile.connection_type === "terminal") {
    return t("profiles.localTerminal")
  }

  if (profile.connection_type === "serial") {
    return normalizeSerialSettings(profile.serial).port
  }

  if (!profile.host) {
    return t("profiles.connectionDetailsUnavailable")
  }

  return `${profile.username ? `${profile.username}@` : ""}${profile.host}${
    profile.port && profile.port !== 22 ? `:${profile.port}` : ""
  }`
}

export const moveProfileInList = (
  profiles: SavedProfile[],
  profileId: string,
  targetGroupId: string | undefined,
  targetProfileId: string | null
) => {
  const sourceIndex = profiles.findIndex((profile) => profile.id === profileId)
  if (sourceIndex < 0) {
    return profiles
  }

  const nextProfiles = [...profiles]
  const [profile] = nextProfiles.splice(sourceIndex, 1)
  const movedProfile = { ...profile, group_id: targetGroupId }
  const targetIndex = targetProfileId
    ? nextProfiles.findIndex((profile) => profile.id === targetProfileId)
    : -1

  nextProfiles.splice(targetIndex >= 0 ? targetIndex : nextProfiles.length, 0, movedProfile)
  return nextProfiles
}
