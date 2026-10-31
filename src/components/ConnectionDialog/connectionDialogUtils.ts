import { SavedProfile } from "@/components/ProfilesPanel"
import {
  normalizeSessionLogPolicy,
  normalizeSshAuthMethod,
  profileTabIcon,
} from "@/lib/profileConnections"
import { normalizeProxyChoice } from "@/lib/proxy"
import { normalizeSerialSettings } from "@/lib/serial"
import { HIGHLIGHT_SET_FOLLOW_DEFAULT } from "@/lib/keywordHighlight/config"
import { defaultShellChoice, defaultShellKey } from "@/lib/localShells"
import { normalizeSftpStartDirectory, type SftpStartDirectory } from "@/lib/sftpStartDirectory"
import { shellIntegrationChoice } from "@/lib/sshShellIntegration"
import type { SessionLogPolicy } from "@/types/tab"

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
    group: profile.group_path ?? "",
    host: profile.host ?? "",
    port: profile.port ?? 22,
    username: profile.username ?? "",
    authMethod,
    agentForward: profile.agent_forward === true,
    privateKeyPath: profile.private_key_path ?? "",
    certificatePath: profile.certificate_path ?? "",
    keepaliveIntervalSecs: profile.keepalive_interval_secs,
    keepaliveCountMax: profile.keepalive_count_max,
    encoding: normalizeTerminalEncoding(profile.encoding),
    terminalTheme: profile.terminal_theme ?? "",
    loginScript: profile.login_script ?? "",
    tabIcon: profileTabIcon(profile).iconStyle,
    distro: profileTabIcon(profile).distroId,
    shellIntegration: shellIntegrationChoice(profile.ssh_shell_integration),
    sessionLog: normalizeSessionLogPolicy(profile.session_log) ?? "default",
    sftpStartDirectory: normalizeSftpStartDirectory(profile.sftp_start_directory) ?? "default",
    highlightSet: profile.highlight_set || HIGHLIGHT_SET_FOLLOW_DEFAULT,
    useJumpHost: profile.use_jump_host ?? jumpHosts.length > 0,
    proxy: normalizeProxyChoice(profile.proxy),
    sudoAutofill: profile.sudo_autofill !== false,
    serial: normalizeSerialSettings(profile.serial),
    jumpHosts: jumpHosts.map((jump) => ({
      ...createDefaultJumpHost(),
      host: jump.host ?? "",
      port: jump.port ?? 22,
      username: jump.username ?? "",
      authMethod: normalizeSshAuthMethod(jump.auth_method),
      privateKeyPath: jump.private_key_path ?? "",
      certificatePath: jump.certificate_path ?? "",
      profileId: jump.profile_id ?? "",
    })),
  }
}

/** The certificate to store or send: only with a key, and omitted when empty. */
export function certificatePayload(
  authMethod: string,
  certificatePath: string
): string | undefined {
  return authMethod === "key" ? certificatePath.trim() || undefined : undefined
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

/** The login script to store or send: omitted when it has no commands. */
export function loginScriptPayload(script: string): string | undefined {
  return script.trim() ? script : undefined
}

/** The form's logging choice as stored: unset follows the logging setting. */
export function sessionLogPayload(
  value: SessionLogPolicy | "default"
): SessionLogPolicy | undefined {
  return value === "default" ? undefined : value
}

/** The form's SFTP start directory as stored: unset follows the SFTP setting. */
export function sftpStartDirectoryPayload(
  value: SftpStartDirectory | "default"
): SftpStartDirectory | undefined {
  return value === "default" ? undefined : value
}

/** The form's highlight set as stored: unset follows the default set. */
export function highlightSetPayload(value: string): string | undefined {
  return value === HIGHLIGHT_SET_FOLLOW_DEFAULT ? undefined : value
}

export function buildInitialForm(
  profile: SavedProfile | null | undefined,
  config: ConfigState
): ConnectionForm {
  const form = buildFormFromProfile(profile)

  if (!profile || form.type === "terminal") {
    const choice = defaultShellChoice(config)
    form.terminalShell = choice.shell
    form.terminalShellCustomPath = choice.customPath ?? ""
    form.terminalShellCustomArgs = choice.customArgs ?? ""
    form.terminalShellId = choice.shellId ?? ""
    form.terminalShellWslDistro = choice.wslDistro ?? ""
    form.terminalShellKey = defaultShellKey(config)
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
    case "serial":
      return form.serial.port.trim() || "Serial port"
  }
}
