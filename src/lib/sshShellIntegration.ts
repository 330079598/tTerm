import type { ShellIntegrationUnavailableReason } from "@/components/TerminalTab/types"

/** A connection's shell integration as its form shows it: `default` follows the setting. */
export type ShellIntegrationChoice = "default" | "on" | "off"

/** The form's choice for a stored or sent value, where missing follows the setting. */
export function shellIntegrationChoice(value: boolean | undefined): ShellIntegrationChoice {
  return value === undefined ? "default" : value ? "on" : "off"
}

/** The form's choice as stored or sent: unset follows the setting. */
export function shellIntegrationPayload(choice: ShellIntegrationChoice): boolean | undefined {
  return choice === "default" ? undefined : choice === "on"
}

/**
 * Whether an SSH connection's sessions start with tTerm's shell integration:
 * its own choice, or the setting when it has none. The backend resolves it the
 * same way (`normalize_ssh` in src-tauri/src/core/session/mod.rs).
 */
export function sshShellIntegrationEnabled(choice: boolean | undefined, setting: boolean): boolean {
  return choice ?? setting
}

/**
 * Whether a session that fell back to a plain shell should say so in a toast.
 * A host that answered for itself (another shell, no `sh`, a cache it cannot
 * write) answers the same every time, which is news only to a connection that
 * turned shell integration on itself; with the setting on by default, a router
 * or a Windows server would otherwise raise a toast on every connect. The
 * connection header still says why.
 */
export function announcesShellIntegrationFallback(
  choice: boolean | undefined,
  reason: ShellIntegrationUnavailableReason
): boolean {
  if (choice === true) return true
  switch (reason.code) {
    case "shell":
    case "noReply":
    case "writeFailed":
    case "path":
      return false
    default:
      return true
  }
}
