import {
  highlightSetPayload,
  loginScriptPayload,
  sessionLogPayload,
} from "@/components/ConnectionDialog/connectionDialogUtils"
import type { ConnectionForm } from "@/components/ConnectionDialog/types"
import {
  KEEPALIVE_COUNT_RANGE,
  KEEPALIVE_INTERVAL_RANGE,
} from "@/components/ConnectionDialog/types"
import type { AppConfig } from "@/lib/appConfig"
import { defaultLocalProfileName, localTerminalSettings } from "@/lib/localProfiles"
import {
  defaultShellUpdates,
  localShellConnection,
  newLocalShellId,
  programName,
  savedLocalShell,
  type LocalShellChoice,
  type ShellConfig,
} from "@/lib/localShells"
import type { SavedProfile, Tab } from "@/types/tab"

/** The key of the choice that types a shell's program in. */
export const NEW_SHELL_KEY = "new"

/** What submitting a local terminal form starts, and the settings it changes. */
export interface LocalShellSubmit {
  choice: LocalShellChoice
  connection: NonNullable<Tab["connection"]>
  updates: Partial<AppConfig> | null
}

/**
 * The local tab a terminal form opens, and the settings it changes: a shell
 * typed in added to the user's shells, and the default shell. Making a shell
 * typed in the default adds it too, since the default is kept by its id.
 */
export function buildLocalShellSubmit(form: ConnectionForm, config: ShellConfig): LocalShellSubmit {
  const path = form.terminalShellCustomPath.trim()
  const args = form.terminalShellCustomArgs.trim()
  const custom = form.terminalShell === "custom"
  let choice: LocalShellChoice = {
    shell: form.terminalShell,
    customPath: custom ? path : undefined,
    customArgs: custom ? args || undefined : undefined,
    shellId: custom ? form.terminalShellId || undefined : undefined,
    wslDistro: form.terminalShell === "wsl" ? form.terminalShellWslDistro || undefined : undefined,
  }
  let updates: Partial<AppConfig> | null = null

  if (form.terminalShellKey === NEW_SHELL_KEY && (form.saveShell || form.makeDefaultShell)) {
    const shell = { id: newLocalShellId(), name: programName(path), path, args }
    choice = { ...choice, shellId: shell.id }
    updates = savedLocalShell(config, shell)
  }
  if (form.makeDefaultShell) {
    updates = { ...updates, ...defaultShellUpdates(choice) }
  }
  const connection: NonNullable<Tab["connection"]> = {
    ...localShellConnection(choice, form.startDirectory.trim() || undefined),
    terminalTheme: form.terminalTheme || undefined,
    loginScript: loginScriptPayload(form.loginScript),
    sessionLog: sessionLogPayload(form.sessionLog),
    highlightSet: highlightSetPayload(form.highlightSet),
  }
  return { choice, connection, updates }
}

/** The name a local terminal is saved under when the form leaves it empty. */
export function localProfileName(form: ConnectionForm): string {
  const shell =
    form.terminalShell === "custom"
      ? programName(form.terminalShellCustomPath)
      : form.terminalShell === "wsl"
        ? form.terminalShellWslDistro || "WSL"
        : form.terminalShell === "auto"
          ? ""
          : form.terminalShell
  return defaultLocalProfileName(form.startDirectory, shell)
}

/** A local terminal as saved, with only what a local profile keeps. */
export function buildLocalTerminalProfile(
  form: ConnectionForm,
  identity: Pick<SavedProfile, "id" | "name" | "group_path">,
  choice: LocalShellChoice
): SavedProfile {
  return {
    ...identity,
    connection_type: "terminal",
    remember_password: false,
    keepalive_interval_secs: KEEPALIVE_INTERVAL_RANGE.fallback,
    keepalive_count_max: KEEPALIVE_COUNT_RANGE.fallback,
    terminal_theme: form.terminalTheme || undefined,
    login_script: loginScriptPayload(form.loginScript),
    session_log: sessionLogPayload(form.sessionLog),
    highlight_set: highlightSetPayload(form.highlightSet),
    local_terminal: localTerminalSettings(choice, form.startDirectory),
  }
}
