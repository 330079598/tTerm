import type { TerminalShellType } from "@/types/tab"

/** A local shell, as `list_available_terminal_shells` reports it. */
export interface TerminalShellProfile {
  shell: TerminalShellType
  label: string
  /** The executable, or a placeholder (`manual path`, `$SHELL`) for an entry to fill in. */
  source: string
}

const terminalShellTranslationKeys: Record<TerminalShellType, string> = {
  auto: "auto",
  cmd: "cmd",
  powershell: "powershell",
  pwsh: "pwsh",
  wsl: "wsl",
  "git-bash": "gitBash",
  custom: "custom",
}

/** A `custom` entry for a shell found on this machine, rather than one to type a path into. */
export function isDetectedCustomShell(profile: TerminalShellProfile): boolean {
  return (
    profile.shell === "custom" && profile.source !== "manual path" && profile.source !== "$SHELL"
  )
}

export function terminalShellLabel(
  profile: TerminalShellProfile,
  t: (key: string, defaultValue: string) => string
): string {
  return isDetectedCustomShell(profile)
    ? profile.label
    : t(
        `connection.terminalShellOptions.${terminalShellTranslationKeys[profile.shell]}`,
        profile.label
      )
}
