import { SavedProfile } from "@/components/ProfilesPanel"
import { normalizeSshAuthMethod } from "@/lib/profileConnections"
import type { TerminalPalette } from "@/types/theme"

import {
  ConfigState,
  ConnectionForm,
  ConnectionType,
  createDefaultJumpHost,
  DEFAULT_TERMINAL_ENCODING,
  defaultForm,
  KEEPALIVE_COUNT_RANGE,
  KEEPALIVE_INTERVAL_RANGE,
  TERMINAL_ENCODINGS,
  TerminalEncoding,
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
    encoding: normalizeTerminalEncoding(profile.encoding),
    terminalTheme: profile.terminal_theme ?? "",
    loginScript: profile.login_script ?? "",
    shellIntegration: profile.shell_integration === true,
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

/** A saved charset label as one the dialog offers; anything else is UTF-8. */
export function normalizeTerminalEncoding(value: string | null | undefined): TerminalEncoding {
  const label = value?.trim().toLowerCase()
  return TERMINAL_ENCODINGS.find((encoding) => encoding === label) ?? DEFAULT_TERMINAL_ENCODING
}

/** The charset to store or send: omitted for UTF-8, the default. */
export function encodingPayload(encoding: TerminalEncoding): string | undefined {
  return encoding === DEFAULT_TERMINAL_ENCODING ? undefined : encoding
}

export interface ThemePickerGroup {
  /** Heading of the group; `null` for entries listed above every group. */
  label: string | null
  themes: Array<{ id: string; label: string; palette?: TerminalPalette }>
}

/**
 * The themes whose name holds every word of `query`, ignoring case; groups
 * left empty are dropped.
 */
export function filterThemeGroups(
  groups: readonly ThemePickerGroup[],
  query: string
): ThemePickerGroup[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean)
  return groups
    .map((group) => ({
      ...group,
      themes: group.themes.filter((theme) => {
        const name = theme.label.toLowerCase()
        return words.every((word) => name.includes(word))
      }),
    }))
    .filter((group) => group.themes.length > 0)
}

/** The login script to store or send: omitted when it has no commands. */
export function loginScriptPayload(script: string): string | undefined {
  return script.trim() ? script : undefined
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
      // Unnamed local tabs are named after their shell.
      return "Shell name"
    case "ssh":
      return form.host ? `${form.username}@${form.host}` : "SSH Connection"
  }
}
