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

  return {
    ...defaultForm,
    type: profile.connection_type as ConnectionType,
    title: profile.name,
    group: profile.group ?? "",
    host: profile.host ?? "",
    port: profile.port ?? 22,
    username: profile.username ?? "",
    authMethod: (profile.auth_method as "password" | "key") ?? "password",
    privateKeyPath: profile.private_key_path ?? "",
    reconnect: profile.reconnect,
    reconnectDelaySecs: profile.reconnect_delay_secs,
    reconnectMaxDelaySecs: profile.reconnect_max_delay_secs,
    reconnectMaxRetries: profile.reconnect_max_retries,
    keepaliveIntervalSecs: profile.keepalive_interval_secs,
    keepaliveCountMax: profile.keepalive_count_max,
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
