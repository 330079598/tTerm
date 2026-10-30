import type { AppConfig, LocalShell } from "@/lib/appConfig"
import {
  isDetectedCustomShell,
  terminalShellLabel,
  type TerminalShellProfile,
} from "@/lib/terminalShells"
import type { Tab, TerminalShellType } from "@/types/tab"

/** The settings that say which shells local tabs can start and which is the default. */
export type ShellConfig = Pick<
  AppConfig,
  | "terminal_shell"
  | "terminal_shell_custom_path"
  | "terminal_shell_custom_args"
  | "terminal_shell_id"
  | "terminal_shell_wsl_distro"
  | "local_shells"
  | "hidden_shells"
>

/** The part of the settings that names the default shell. */
export type DefaultShellSettings = Pick<
  AppConfig,
  | "terminal_shell"
  | "terminal_shell_custom_path"
  | "terminal_shell_custom_args"
  | "terminal_shell_id"
  | "terminal_shell_wsl_distro"
>

/** A shell a local tab can start, as a tab is opened with it. */
export interface LocalShellChoice {
  shell: TerminalShellType
  /** The executable of a `custom` shell. */
  customPath?: string
  customArgs?: string
  /** The user's shell this is, so a restored tab follows its later edits. */
  shellId?: string
  /** The WSL distribution a `wsl` shell starts; WSL's default when missing. */
  wslDistro?: string
}

/** One shell in the lists: Auto, one found on this machine, or one the user added. */
export interface ShellEntry {
  /** `auto`, a detected shell's key (`pwsh`, `wsl:Ubuntu`, or its path), or `custom:<id>`. */
  key: string
  kind: "auto" | "detected" | "custom"
  label: string
  /** The executable, with the arguments of a shell the user added. */
  detail: string
  /** The executable alone, or nothing for Auto. */
  source: string
  choice: LocalShellChoice
  hidden: boolean
}

type Translate = (key: string, defaultValue: string) => string

export const AUTO_SHELL_KEY = "auto"

/**
 * The key a detected shell is hidden by: its kind on Windows, or a WSL
 * distribution's name, and its path elsewhere.
 */
export function detectedShellKey(profile: TerminalShellProfile): string {
  if (profile.wslDistro) return wslDistroKey(profile.wslDistro)
  return profile.shell === "custom" ? profile.source : profile.shell
}

function wslDistroKey(name: string): string {
  return `wsl:${name}`
}

/** The `wsl.exe` arguments that start a distribution. */
function wslDistroArgs(name: string): string {
  return `-d ${name}`
}

export function customShellKey(id: string): string {
  return `custom:${id}`
}

/** The program a path names, without directories or extension: `nu` for `C:\nu\nu.exe`. */
export function programName(path: string): string {
  const file = path.trim().replace(/^"|"$/g, "").split(/[\\/]/).pop() ?? ""
  return file.replace(/\.[^.]+$/, "") || file
}

function describeShell(shell: LocalShell): string {
  const args = shell.args.trim()
  return args ? `${shell.path.trim()} ${args}` : shell.path.trim()
}

function samePath(a: string, b: string): boolean {
  return a.trim() === b.trim()
}

/**
 * Every shell a local tab can start, in the order the lists show them: Auto,
 * the shells found on this machine, then the ones the user added. A detected
 * shell the user also added as it is shows once, as theirs.
 */
export function buildShellEntries(
  detected: readonly TerminalShellProfile[],
  config: Pick<
    ShellConfig,
    "terminal_shell" | "terminal_shell_wsl_distro" | "local_shells" | "hidden_shells"
  >,
  t: Translate
): ShellEntry[] {
  const entries: ShellEntry[] = [
    {
      key: AUTO_SHELL_KEY,
      kind: "auto",
      label: t("connection.terminalShellOptions.auto", "Auto (recommended)"),
      detail: "",
      source: "",
      choice: { shell: "auto" },
      hidden: false,
    },
  ]
  // While the default is WSL's own default distribution, that distribution's
  // entry stands for it, so the default is marked on the shell it starts.
  const wslDefault = config.terminal_shell === "wsl" && !config.terminal_shell_wsl_distro.trim()
  const addedAsIs = (path: string) =>
    config.local_shells.some((shell) => !shell.args.trim() && samePath(shell.path, path))
  for (const profile of detected) {
    if (profile.shell === "auto") continue
    if (
      profile.shell === "custom" &&
      (!isDetectedCustomShell(profile) || addedAsIs(profile.source))
    )
      continue
    const key = wslDefault && profile.defaultWslDistro ? "wsl" : detectedShellKey(profile)
    entries.push({
      key,
      kind: "detected",
      label: terminalShellLabel(profile, t),
      detail: profile.wslDistro
        ? `${profile.source} ${wslDistroArgs(profile.wslDistro)}`
        : profile.source,
      source: profile.source,
      choice: {
        shell: profile.shell,
        customPath: profile.shell === "custom" ? profile.source : undefined,
        wslDistro: profile.wslDistro,
      },
      hidden: config.hidden_shells.includes(key),
    })
  }
  for (const shell of config.local_shells) {
    entries.push({
      key: customShellKey(shell.id),
      kind: "custom",
      label: shell.name.trim() || programName(shell.path),
      detail: describeShell(shell),
      source: shell.path.trim(),
      choice: ownShellChoice(shell),
      hidden: false,
    })
  }
  return entries
}

/** The program and arguments a shell of the user's own copied from a detected `entry` starts. */
export function copiedShellCommand(entry: ShellEntry): { path: string; args: string } {
  const distro = entry.choice.wslDistro
  return { path: entry.source, args: distro ? wslDistroArgs(distro) : "" }
}

/** The key of the default shell among {@link buildShellEntries}. */
export function defaultShellKey(config: ShellConfig): string {
  const id = config.terminal_shell_id
  if (id && config.local_shells.some((shell) => shell.id === id)) return customShellKey(id)
  const distro = config.terminal_shell_wsl_distro.trim()
  if (config.terminal_shell === "wsl" && distro) return wslDistroKey(distro)
  if (config.terminal_shell === "custom") {
    const path = config.terminal_shell_custom_path.trim()
    const added = config.local_shells.find(
      (shell) => !shell.args.trim() && samePath(shell.path, path)
    )
    return added ? customShellKey(added.id) : path
  }
  return config.terminal_shell
}

/** The settings that make `choice` the default shell. */
export function defaultShellUpdates(choice: LocalShellChoice): DefaultShellSettings {
  const custom = choice.shell === "custom"
  return {
    terminal_shell: choice.shell,
    terminal_shell_custom_path: custom ? (choice.customPath ?? "").trim() : "",
    terminal_shell_custom_args: custom ? (choice.customArgs ?? "").trim() : "",
    terminal_shell_id: custom ? (choice.shellId ?? "") : "",
    terminal_shell_wsl_distro: choice.shell === "wsl" ? (choice.wslDistro ?? "").trim() : "",
  }
}

/** The default shell, with the current path of a shell the user added. */
export function defaultShellChoice(config: ShellConfig): LocalShellChoice {
  const added = config.local_shells.find((shell) => shell.id === config.terminal_shell_id)
  if (config.terminal_shell === "custom" && added) return ownShellChoice(added)
  const custom = config.terminal_shell === "custom"
  return {
    shell: config.terminal_shell,
    customPath: custom ? config.terminal_shell_custom_path.trim() : undefined,
    customArgs: custom ? config.terminal_shell_custom_args.trim() || undefined : undefined,
    wslDistro:
      config.terminal_shell === "wsl"
        ? config.terminal_shell_wsl_distro.trim() || undefined
        : undefined,
  }
}

/** A local tab's connection that starts `choice` in `cwd`. */
export function localShellConnection(
  choice: LocalShellChoice,
  cwd?: string
): NonNullable<Tab["connection"]> {
  const custom = choice.shell === "custom"
  return {
    type: "terminal",
    cwd,
    terminalShell: choice.shell,
    terminalShellCustomPath: custom ? choice.customPath : undefined,
    terminalShellCustomArgs: custom ? choice.customArgs : undefined,
    terminalShellId: custom ? choice.shellId : undefined,
    terminalShellWslDistro: choice.shell === "wsl" ? choice.wslDistro : undefined,
  }
}

/**
 * A saved local tab's connection with the current path and arguments of the
 * user's shell it starts. One whose shell was since removed keeps the path it
 * was saved with.
 */
export function withCurrentLocalShell(
  connection: Tab["connection"],
  shells: readonly LocalShell[]
): Tab["connection"] {
  const id = connection?.terminalShellId
  const shell = id ? shells.find((candidate) => candidate.id === id) : undefined
  if (!connection || !shell) return connection
  return {
    ...connection,
    terminalShell: "custom",
    terminalShellCustomPath: shell.path.trim(),
    terminalShellCustomArgs: shell.args.trim() || undefined,
  }
}

/** The settings with `shell` added or, when its id is already listed, replaced. */
export function savedLocalShell(
  config: ShellConfig,
  shell: LocalShell
): Pick<AppConfig, "local_shells"> & Partial<DefaultShellSettings> {
  const listed = config.local_shells.some((candidate) => candidate.id === shell.id)
  const local_shells = listed
    ? config.local_shells.map((candidate) => (candidate.id === shell.id ? shell : candidate))
    : [...config.local_shells, shell]
  // The default's own path and arguments follow its edits, for older versions.
  if (config.terminal_shell === "custom" && config.terminal_shell_id === shell.id) {
    return {
      local_shells,
      terminal_shell_custom_path: shell.path.trim(),
      terminal_shell_custom_args: shell.args.trim(),
    }
  }
  return { local_shells }
}

/** Starting one of the user's own shells. */
export function ownShellChoice(shell: LocalShell): LocalShellChoice {
  return {
    shell: "custom",
    customPath: shell.path.trim(),
    customArgs: shell.args.trim() || undefined,
    shellId: shell.id,
  }
}

/** The settings with `shell` saved, and made the default when asked. */
export function savedLocalShellUpdates(
  config: ShellConfig,
  shell: LocalShell,
  makeDefault: boolean
): Partial<AppConfig> {
  return {
    ...savedLocalShell(config, shell),
    ...(makeDefault ? defaultShellUpdates(ownShellChoice(shell)) : {}),
  }
}

/** Whether a local tab's connection starts the same shell as `choice`. */
export function startsShell(connection: Tab["connection"], choice: LocalShellChoice): boolean {
  const shell = connection?.terminalShell ?? "auto"
  if (shell !== choice.shell) return false
  if (shell === "wsl")
    return (connection?.terminalShellWslDistro ?? "") === (choice.wslDistro ?? "")
  if (shell !== "custom") return true
  if (connection?.terminalShellId || choice.shellId) {
    return connection?.terminalShellId === choice.shellId
  }
  return samePath(connection?.terminalShellCustomPath ?? "", choice.customPath ?? "")
}

/** Whether a local tab's connection starts `shell` with its saved path and arguments. */
export function startsAsSaved(connection: Tab["connection"], shell: LocalShell): boolean {
  return (
    samePath(connection?.terminalShellCustomPath ?? "", shell.path) &&
    (connection?.terminalShellCustomArgs ?? "").trim() === shell.args.trim()
  )
}

/** A local tab started again with `choice`, in the directory its shell was in. */
export function withLocalShell<T extends Pick<Tab, "connection" | "sessionNonce">>(
  tab: T,
  choice: LocalShellChoice
): T {
  return {
    ...tab,
    connection: localShellConnection(choice, tab.connection?.cwd),
    sessionNonce: ((tab.sessionNonce ?? 0) + 1) >>> 0,
  }
}

/** What a local tab whose shell failed to start can open instead. */
export interface FailedShellFallback {
  choice: LocalShellChoice
  /** Whether this is Auto standing in for a default shell that failed itself. */
  replacesDefault: boolean
}

/**
 * The shell a tab whose shell failed to start can open instead: the default,
 * or Auto when the default is the shell that failed, so a broken default
 * still leaves a way out. Nothing when Auto itself failed.
 */
export function failedShellFallback(
  connection: Tab["connection"],
  config: ShellConfig
): FailedShellFallback | undefined {
  const fallback = defaultShellChoice(config)
  if (!startsShell(connection, fallback)) return { choice: fallback, replacesDefault: false }
  if (fallback.shell === "auto") return undefined
  return { choice: { shell: "auto" }, replacesDefault: true }
}

/** The settings without the user's shell `id`; Auto becomes the default if it was. */
export function removedLocalShell(
  config: ShellConfig,
  id: string
): Pick<AppConfig, "local_shells"> & Partial<DefaultShellSettings> {
  const local_shells = config.local_shells.filter((shell) => shell.id !== id)
  if (config.terminal_shell === "custom" && config.terminal_shell_id === id) {
    return { local_shells, ...defaultShellUpdates({ shell: "auto" }) }
  }
  return { local_shells }
}

/** The user's shells with `id` moved one place up or down. */
export function movedLocalShell(
  shells: readonly LocalShell[],
  id: string,
  offset: -1 | 1
): LocalShell[] {
  const index = shells.findIndex((shell) => shell.id === id)
  const target = index + offset
  if (index < 0 || target < 0 || target >= shells.length) return [...shells]
  const next = [...shells]
  ;[next[index], next[target]] = [next[target], next[index]]
  return next
}

/** The hidden shells with `key` shown again, or hidden. */
export function toggledHiddenShell(hidden: readonly string[], key: string): string[] {
  return hidden.includes(key) ? hidden.filter((other) => other !== key) : [...hidden, key]
}

/** A new id for a shell the user adds. */
export function newLocalShellId(): string {
  return crypto.randomUUID()
}
