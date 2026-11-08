/** Compare hook and live shell directories without guessing a missing directory. */
export function agentResumeCwdMatches(hookCwd: string | undefined, shellCwd: string | undefined) {
  if (!hookCwd || !shellCwd || /[\p{Cc}]/u.test(hookCwd + shellCwd)) return false
  const normalize = (path: string) => {
    const slashes = path.replaceAll("\\", "/")
    const unprefixed = slashes.replace(/^\/\/\?\/(?:UNC\/)?/i, (prefix) =>
      /UNC/i.test(prefix) ? "//" : ""
    )
    // Infer Windows paths from their syntax so an SSH POSIX path stays case-sensitive.
    return /^[a-z]:\//i.test(unprefixed) || unprefixed.startsWith("//")
      ? unprefixed.toLowerCase()
      : path
  }
  return normalize(hookCwd) === normalize(shellCwd)
}
