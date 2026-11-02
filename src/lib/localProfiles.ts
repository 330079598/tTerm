import type { LocalShellChoice } from "@/lib/localShells"
import type { LocalTerminalSettings, SavedProfile, Tab } from "@/types/tab"

/** The name a saved local terminal gets when none is typed. */
const FALLBACK_NAME = "Terminal"

/** The shell a saved local terminal starts; Auto when it names none. */
export function localProfileChoice(settings: LocalTerminalSettings | undefined): LocalShellChoice {
  const shell = settings?.shell ?? "auto"
  const custom = shell === "custom"
  return {
    shell,
    customPath: custom ? settings?.customPath : undefined,
    customArgs: custom ? settings?.customArgs : undefined,
    shellId: custom ? settings?.shellId : undefined,
    wslDistro: shell === "wsl" ? settings?.wslDistro : undefined,
  }
}

/** What a saved local terminal keeps: `choice` started in `startDirectory`. */
export function localTerminalSettings(
  choice: LocalShellChoice,
  startDirectory: string
): LocalTerminalSettings {
  const custom = choice.shell === "custom"
  const text = (value: string | undefined) => value?.trim() || undefined
  return {
    shell: choice.shell,
    customPath: custom ? text(choice.customPath) : undefined,
    customArgs: custom ? text(choice.customArgs) : undefined,
    shellId: custom ? text(choice.shellId) : undefined,
    wslDistro: choice.shell === "wsl" ? text(choice.wslDistro) : undefined,
    startDirectory: text(startDirectory),
  }
}

/**
 * The name for a saved local terminal left unnamed: the last folder of the
 * directory it starts in, else its shell's name.
 */
export function defaultLocalProfileName(startDirectory: string, shellName: string): string {
  const path = startDirectory.trim().replace(/[\\/]+$/, "")
  const folder = path.slice(Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\")) + 1)
  if (folder && folder !== "~" && !/^[A-Za-z]:$/.test(folder)) return folder
  return shellName.trim() || FALLBACK_NAME
}

/**
 * A new saved local terminal from an open local tab: its shell, the
 * directory its shell is in now, and the tab's name.
 */
export function draftLocalProfileFromTab(tab: Tab): SavedProfile {
  const connection = tab.connection
  const choice = localProfileChoice({
    shell: connection?.terminalShell ?? "auto",
    customPath: connection?.terminalShellCustomPath,
    customArgs: connection?.terminalShellCustomArgs,
    shellId: connection?.terminalShellId,
    wslDistro: connection?.terminalShellWslDistro,
  })
  return {
    id: "",
    name: tab.title,
    connection_type: "terminal",
    remember_password: false,
    keepalive_interval_secs: 15,
    keepalive_count_max: 3,
    terminal_theme: connection?.terminalTheme,
    highlight_set: connection?.highlightSet,
    session_log: connection?.sessionLog,
    login_script: connection?.loginScript,
    local_terminal: localTerminalSettings(choice, connection?.cwd ?? ""),
  }
}
