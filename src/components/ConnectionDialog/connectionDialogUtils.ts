import { SavedProfile } from "@/components/ProfilesPanel"

import {
  ConfigState,
  ConnectionForm,
  ConnectionType,
  defaultForm,
} from "@/components/ConnectionDialog/types"

export function buildFormFromProfile(profile?: SavedProfile | null): ConnectionForm {
  if (!profile) {
    return { ...defaultForm }
  }

  const jump = profile.jump_host
  const authMethod = profile.auth_method === "key" ? "key" : "password"
  const jumpAuthMethod = jump?.auth_method === "key" ? "key" : "password"

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
    // Jump host fields from saved profile
    useJumpHost: !!jump,
    jumpHost: jump?.host ?? "",
    jumpPort: jump?.port ?? 22,
    jumpUsername: jump?.username ?? "",
    jumpAuthMethod,
    jumpPrivateKeyPath: jump?.private_key_path ?? "",
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
