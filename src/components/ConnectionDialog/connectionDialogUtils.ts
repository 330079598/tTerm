import { SavedProfile } from "@/components/ProfilesPanel"
import { normalizeSshAuthMethod } from "@/lib/profileConnections"

import {
  ConfigState,
  ConnectionForm,
  ConnectionType,
  createDefaultJumpHost,
  defaultForm,
  KEEPALIVE_COUNT_RANGE,
  KEEPALIVE_INTERVAL_RANGE,
} from "@/components/ConnectionDialog/types"

export function buildFormFromProfile(profile?: SavedProfile | null): ConnectionForm {
  if (!profile) {
    return { ...defaultForm }
  }

  const jumpHosts = profile.jump_hosts ?? []
  const authMethod = normalizeSshAuthMethod(profile.auth_method)

  return {
    ...defaultForm,
    type: profile.connection_type as ConnectionType,
    title: profile.name,
    group: profile.group ?? "",
    host: profile.host ?? "",
    port: profile.port ?? 22,
    username: profile.username ?? "",
    authMethod,
    agentForward: profile.agent_forward === true,
    privateKeyPath: profile.private_key_path ?? "",
    keepaliveIntervalSecs: profile.keepalive_interval_secs,
    keepaliveCountMax: profile.keepalive_count_max,
    useJumpHost: profile.use_jump_host ?? jumpHosts.length > 0,
    sudoAutofill: profile.sudo_autofill !== false,
    jumpHosts: jumpHosts.map((jump) => ({
      ...createDefaultJumpHost(),
      host: jump.host ?? "",
      port: jump.port ?? 22,
      username: jump.username ?? "",
      authMethod: normalizeSshAuthMethod(jump.auth_method),
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

function clampToRange(
  value: number,
  range: { min: number; max: number; fallback: number }
): number {
  if (!Number.isFinite(value) || value <= 0) return range.fallback
  return Math.min(range.max, Math.max(range.min, Math.round(value)))
}

/** Keepalive settings as the backend accepts them; an empty field falls back to the default. */
export function normalizeKeepalive(
  form: Pick<ConnectionForm, "keepaliveIntervalSecs" | "keepaliveCountMax">
): { intervalSecs: number; countMax: number } {
  return {
    intervalSecs: clampToRange(form.keepaliveIntervalSecs, KEEPALIVE_INTERVAL_RANGE),
    countMax: clampToRange(form.keepaliveCountMax, KEEPALIVE_COUNT_RANGE),
  }
}

export function getDefaultTitle(type: ConnectionType, form: ConnectionForm): string {
  switch (type) {
    case "terminal":
      return "OS terminal"
    case "ssh":
      return form.host ? `${form.username}@${form.host}` : "SSH Connection"
  }
}
