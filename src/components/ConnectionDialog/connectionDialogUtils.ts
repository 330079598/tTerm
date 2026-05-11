import { SavedProfile } from "@/components/ProfilesPanel"

import {
  ConfigState,
  ConnectionForm,
  ConnectionType,
  createDefaultJumpHost,
  defaultForm,
} from "@/components/ConnectionDialog/types"

export function buildFormFromProfile(profile?: SavedProfile | null): ConnectionForm {
  if (!profile) {
    return { ...defaultForm }
  }

  const legacyJump = profile.jump_host
  const jumpHosts = profile.jump_hosts?.length ? profile.jump_hosts : legacyJump ? [legacyJump] : []
  const authMethod = profile.auth_method === "key" ? "key" : "password"

  return {
    ...defaultForm,
    type: profile.connection_type as ConnectionType,
    title: profile.name,
    group: profile.group ?? "",
    host: profile.host ?? "",
    port: profile.port ?? 22,
    username: profile.username ?? "",
    authMethod,
    privateKeyPath: profile.private_key_path ?? "",
    keepaliveIntervalSecs: profile.keepalive_interval_secs,
    keepaliveCountMax: profile.keepalive_count_max,
    useJumpHost: jumpHosts.length > 0,
    jumpHosts: jumpHosts.map((jump) => ({
      ...createDefaultJumpHost(),
      host: jump.host ?? "",
      port: jump.port ?? 22,
      username: jump.username ?? "",
      authMethod: jump.auth_method === "key" ? "key" : "password",
      privateKeyPath: jump.private_key_path ?? "",
    })),
  }
}

export function buildInitialForm(
  profile: SavedProfile | null | undefined,
  config: ConfigState
): ConnectionForm {
  const form = buildFormFromProfile(profile)

  if (!profile || form.type === "terminal") {
    form.terminalShell = config.terminal_shell
    form.terminalShellCustomPath = config.terminal_shell_custom_path
    form.terminalShellCustomArgs = config.terminal_shell_custom_args
  }

  return form
}

export function getDefaultTitle(type: ConnectionType, form: ConnectionForm): string {
  switch (type) {
    case "terminal":
      return "OS terminal"
    case "ssh":
      return form.host ? `${form.username}@${form.host}` : "SSH Connection"
  }
}
