import type { SavedProfile } from "@/types/tab"

export const UNGROUPED_KEY = "__ungrouped__"

export const getGroupKey = (group: string) => (group.trim() ? group.trim() : UNGROUPED_KEY)
export const getGroupNameFromKey = (key: string) => (key === UNGROUPED_KEY ? "" : key)
export const profileDragId = (profileId: string) => `profile:${profileId}:drag`
export const profileDropId = (profileId: string) => `profile:${profileId}:drop`
export const groupDropId = (groupKey: string) =>
  `profile-group:${encodeURIComponent(groupKey)}:drop`
export const groupListId = (groupKey: string) =>
  `profile-group-${encodeURIComponent(groupKey)}-list`

export const getProfileIdFromDndId = (id?: string | number | null): string | null => {
  if (!id) return null
  const parts = String(id).split(":")
  if (parts.length < 3 || parts[0] !== "profile") return null
  return parts[1] || null
}

export const getGroupKeyFromDndId = (id?: string | number | null): string | null => {
  if (!id) return null
  const parts = String(id).split(":")
  if (parts.length < 3 || parts[0] !== "profile-group") return null
  return parts[1] ? decodeURIComponent(parts[1]) : null
}

export const getProfileDropIdFromDndId = (id?: string | number | null): string | null => {
  if (!id) return null
  const parts = String(id).split(":")
  if (parts.length < 3 || parts[0] !== "profile" || parts[2] !== "drop") return null
  return parts[1] || null
}

export const buildProfileSubtitle = (
  profile: SavedProfile,
  t: (key: string, options?: Record<string, unknown>) => string
) => {
  if (profile.connection_type === "terminal") {
    return t("profiles.localTerminal")
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
  targetGroup: string,
  targetProfileId: string | null
) => {
  const sourceIndex = profiles.findIndex((profile) => profile.id === profileId)
  if (sourceIndex < 0) {
    return profiles
  }

  const nextProfiles = [...profiles]
  const [profile] = nextProfiles.splice(sourceIndex, 1)
  const movedProfile = { ...profile, group: targetGroup }
  const targetIndex = targetProfileId
    ? nextProfiles.findIndex((profile) => profile.id === targetProfileId)
    : -1

  nextProfiles.splice(targetIndex >= 0 ? targetIndex : nextProfiles.length, 0, movedProfile)
  return nextProfiles
}

export interface GroupColumn {
  key: string
  name: string
  profiles: SavedProfile[]
}
