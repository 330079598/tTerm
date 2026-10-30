import { normalizeSshAuthMethod } from "@/lib/profileConnections"
import type { JumpHostForm } from "@/components/ConnectionDialog/types"
import type { SavedProfile } from "@/types/tab"

/** Saved connections listed under one group heading; `group` is empty for ungrouped ones. */
export interface JumpHostCandidateGroup {
  group: string
  profiles: SavedProfile[]
}

/**
 * The saved connections a hop of the connection `currentProfileId` can log in
 * as, grouped as the connection list files them. The connection itself is
 * left out: it cannot be a jump host on its own way.
 */
export function jumpHostCandidateGroups(
  profiles: SavedProfile[],
  currentProfileId: string
): JumpHostCandidateGroup[] {
  const groups = new Map<string, SavedProfile[]>()
  for (const profile of profiles) {
    if (profile.connection_type !== "ssh" || profile.id === currentProfileId) continue
    if (!profile.host?.trim() || !profile.username?.trim()) continue
    const group = profile.group_path ?? ""
    groups.set(group, [...(groups.get(group) ?? []), profile])
  }
  return [...groups].map(([group, members]) => ({ group, profiles: members }))
}

/** `user@host:port` of a saved connection. */
export function profileAddress(profile: SavedProfile): string {
  return `${profile.username ?? ""}@${profile.host ?? ""}:${profile.port ?? 22}`
}

/**
 * What a hop becomes when it logs in as `profile`: its id, and a copy of its
 * address and login for display and for versions that don't read the id.
 * Anything typed for the hop before is dropped.
 */
export function jumpHostFromProfile(profile: SavedProfile): Partial<JumpHostForm> {
  return {
    profileId: profile.id,
    host: profile.host?.trim() ?? "",
    port: profile.port ?? 22,
    username: profile.username?.trim() ?? "",
    authMethod: normalizeSshAuthMethod(profile.auth_method),
    password: "",
    privateKeyPath: profile.private_key_path ?? "",
    privateKeyPassphrase: "",
    certificatePath: profile.certificate_path ?? "",
  }
}
