import type { ConnectionType, SavedProfile, Tab } from "@/types/tab"

export function buildConnectionFromProfile(profile: SavedProfile): Omit<Tab, "id" | "isActive"> {
  const connectionType = profile.connection_type as ConnectionType
  const jumpHosts = profile.jump_hosts ?? []

  return {
    title: profile.name,
    type: connectionType,
    isModified: false,
    connection: {
      type: connectionType,
      profileId: profile.id,
      profileName: profile.name,
      host: profile.host,
      port: profile.port,
      username: profile.username,
      privateKeyPath: profile.auth_method === "key" ? profile.private_key_path : undefined,
      keepaliveIntervalSecs: profile.keepalive_interval_secs,
      keepaliveCountMax: profile.keepalive_count_max,
      serverMonitorVisible: profile.server_monitor_visible === true,
      jumpHosts:
        jumpHosts.length > 0
          ? jumpHosts.map((jump) => ({
              host: jump.host,
              port: jump.port,
              username: jump.username,
              authMethod: jump.auth_method === "key" ? "key" : "password",
              privateKeyPath: jump.private_key_path,
            }))
          : undefined,
    },
  }
}
