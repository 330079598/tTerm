/** How a shell wants a dropped path written. */
export type DropQuoting = "posix" | "powershell" | "cmd" | "wsl" | "git-bash"

/**
 * The quoting for a local shell, from the platform and the shell's name as
 * `local_terminal_shell_name` reports it (`pwsh`, `cmd`, `WSL`, `zsh`).
 * A Windows shell tTerm cannot place gets double quotes, as Windows
 * Terminal gives every shell that is not WSL.
 */
export function dropQuotingFor(platform: string, shellName: string | undefined): DropQuoting {
  if (platform !== "windows") return "posix"
  switch (shellName?.trim().toLowerCase()) {
    case "powershell":
    case "pwsh":
      return "powershell"
    case "wsl":
      return "wsl"
    case "git bash":
      return "git-bash"
    default:
      return "cmd"
  }
}

// Characters a POSIX shell reads as syntax rather than as part of a word.
const POSIX_SPECIAL = /[\s!"#$&'()*;<>?[\\\]^`{|}~=]/g

// Characters that need quotes in cmd, from `cmd /?`, and in PowerShell.
const CMD_SPECIAL = /[\s&()[\]{}^=;!'+,`~%]/
const POWERSHELL_SPECIAL = /[\s'"`$&()[\]{};,@#|<>]/

/** Escapes each special character with a backslash, as macOS Terminal does. */
function quotePosix(path: string): string {
  // A backslash before a line break joins the lines, so a break gets quoted.
  return path.replace(POSIX_SPECIAL, (char) => (char === "\n" ? "'\n'" : `\\${char}`))
}

function quotePowerShell(path: string): string {
  // Single quotes keep `$` and backticks literal, unlike double quotes.
  return POWERSHELL_SPECIAL.test(path) ? `'${path.replace(/'/g, "''")}'` : path
}

function quoteCmd(path: string): string {
  return CMD_SPECIAL.test(path) ? `"${path}"` : path
}

/**
 * A Windows path as a Linux or MSYS shell sees it: `C:\a` becomes
 * `<driveRoot>c/a`, and a path inside a WSL distribution its own path.
 * Any other network path stays as it is, in single quotes.
 */
function toUnixPath(path: string, driveRoot: string): string {
  const drive = /^([A-Za-z]):[\\/]?(.*)$/s.exec(path)
  if (drive) {
    const rest = drive[2].replace(/\\/g, "/")
    return quotePosix(`${driveRoot}${drive[1].toLowerCase()}${rest ? `/${rest}` : ""}`)
  }
  const distro = /^\\\\(?:wsl\$|wsl\.localhost)\\[^\\]+(\\.*)?$/i.exec(path)
  if (distro) return quotePosix((distro[1] ?? "\\").replace(/\\/g, "/"))
  return `'${path.replace(/'/g, `'\\''`)}'`
}

/** One dropped path, written so the shell reads it back as that path. */
export function quoteDroppedPath(path: string, quoting: DropQuoting): string {
  switch (quoting) {
    case "posix":
      return quotePosix(path)
    case "powershell":
      return quotePowerShell(path)
    case "cmd":
      return quoteCmd(path)
    case "wsl":
      return toUnixPath(path, "/mnt/")
    case "git-bash":
      return toUnixPath(path, "/")
  }
}

/**
 * The text a drop of `paths` types into the terminal: each path quoted for
 * the shell, separated by spaces, with a space after the last so the next
 * argument can follow, as macOS Terminal and Windows Terminal do.
 */
export function droppedPathsText(paths: string[], quoting: DropQuoting): string {
  const quoted = paths
    .filter((path) => path.length > 0)
    .map((path) => quoteDroppedPath(path, quoting))
  return quoted.length > 0 ? `${quoted.join(" ")} ` : ""
}
