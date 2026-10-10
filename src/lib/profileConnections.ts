import type {
  ConnectionType,
  SavedProfile,
  SessionLogPolicy,
  SshAuthMethod,
  Tab,
} from "@/types/tab"

/** The auth method of a saved profile or jump host; anything unknown is a password login. */
export function normalizeSshAuthMethod(value: string | null | undefined): SshAuthMethod {
  return value === "key" || value === "agent" || value === "interactive" ? value : "password"
}

/** A stored logging choice; anything unknown follows the logging setting. */
export function normalizeSessionLogPolicy(
  value: string | null | undefined
): SessionLogPolicy | undefined {
  return value === "always" || value === "never" ? value : undefined
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
      sessionLog: normalizeSessionLogPolicy(profile.session_log),
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

/**
 * An open tab of an edited profile, now connecting with what was saved. A
 * password typed for the tab is kept only while it still signs in to the
 * same account; the title follows a rename unless the tab was renamed itself.
 */
export function applyProfileToTab(tab: Tab, profile: SavedProfile): Tab {
  const previous = tab.connection ?? {}
  const next = buildConnectionFromProfile(profile).connection ?? {}
  const sameAccount =
    previous.host === next.host &&
    previous.port === next.port &&
    previous.username === next.username &&
    previous.authMethod === next.authMethod &&
    previous.privateKeyPath === next.privateKeyPath
  const connection = { ...previous, ...next }
  if (!sameAccount) {
    connection.password = undefined
    connection.privateKeyPassphrase = undefined
    connection.ignoreSavedPassword = undefined
  }

  const oldName = previous.profileName
  let { title, duplicateBaseTitle } = tab
  if (oldName && oldName !== profile.name) {
    if (title === oldName) {
      title = profile.name
    } else if (duplicateBaseTitle === oldName && title.startsWith(oldName)) {
      title = profile.name + title.slice(oldName.length)
      duplicateBaseTitle = profile.name
    }
  }

  return { ...tab, title, duplicateBaseTitle, connection }
}
