import type { ConnectionType, SavedProfile, SshAuthMethod, Tab } from "@/types/tab"

/** The auth method of a saved profile or jump host; anything unknown is a password login. */
export function normalizeSshAuthMethod(value: string | null | undefined): SshAuthMethod {
  return value === "key" || value === "agent" || value === "interactive" ? value : "password"
}

export function buildConnectionFromProfile(profile: SavedProfile): Omit<Tab, "id" | "isActive"> {
  const connectionType = profile.connection_type as ConnectionType
  const jumpHosts = profile.jump_hosts ?? []
  const useJumpHost = profile.use_jump_host ?? jumpHosts.length > 0

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
      authMethod: normalizeSshAuthMethod(profile.auth_method),
      agentForward: profile.agent_forward === true,
      privateKeyPath: profile.auth_method === "key" ? profile.private_key_path : undefined,
      keepaliveIntervalSecs: profile.keepalive_interval_secs,
      keepaliveCountMax: profile.keepalive_count_max,
      serverMonitorVisible: profile.server_monitor_visible === true,
      encoding: profile.encoding,
      terminalTheme: profile.terminal_theme,
      loginScript: profile.login_script,
      shellIntegration: profile.shell_integration === true,
      jumpHosts:
        useJumpHost && jumpHosts.length > 0
          ? jumpHosts.map((jump) => ({
              host: jump.host,
              port: jump.port,
              username: jump.username,
              authMethod: normalizeSshAuthMethod(jump.auth_method),
              privateKeyPath: jump.private_key_path,
            }))
          : undefined,
    },
  }
}
