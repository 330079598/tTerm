import type { ConnectionForm } from "@/components/ConnectionDialog/types"
import type { AppConfig } from "@/lib/appConfig"
import {
  defaultShellUpdates,
  localShellConnection,
  newLocalShellId,
  programName,
  savedLocalShell,
  type LocalShellChoice,
  type ShellConfig,
} from "@/lib/localShells"
import type { Tab } from "@/types/tab"

/** The key of the choice that types a shell's program in. */
export const NEW_SHELL_KEY = "new"

/**
 * The local tab a terminal form opens, and the settings it changes: a shell
 * typed in added to the user's shells, and the default shell. Making a shell
 * typed in the default adds it too, since the default is kept by its id.
 */
export function buildLocalShellSubmit(
  form: ConnectionForm,
  config: ShellConfig
): { connection: NonNullable<Tab["connection"]>; updates: Partial<AppConfig> | null } {
  const path = form.terminalShellCustomPath.trim()
  const args = form.terminalShellCustomArgs.trim()
  const custom = form.terminalShell === "custom"
  let choice: LocalShellChoice = {
    shell: form.terminalShell,
    customPath: custom ? path : undefined,
    customArgs: custom ? args || undefined : undefined,
    shellId: custom ? form.terminalShellId || undefined : undefined,
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
  return { connection: localShellConnection(choice), updates }
}
