// Where an SSH tab's SFTP drawer first opens: the user's home directory, the
// last directory listed on that host (the backend keeps it per host, port and
// user), or the directory the terminal's shell reported.

export const SFTP_START_DIRECTORIES = ["home", "last", "terminal"] as const

export type SftpStartDirectory = (typeof SFTP_START_DIRECTORIES)[number]

/** A stored choice, or undefined for anything unknown. */
export function normalizeSftpStartDirectory(
  value: string | null | undefined
): SftpStartDirectory | undefined {
  return SFTP_START_DIRECTORIES.find((choice) => choice === value)
}

/**
 * The path `sftp_list_directory` gets for the first listing: the terminal's
 * directory when the drawer goes there and one is known, `.` (home) or null
 * (the last directory, then home, then `/`).
 */
export function sftpStartPath(
  choice: SftpStartDirectory,
  followsTerminal: boolean,
  terminalCwd: string | null
): string | null {
  if ((choice === "terminal" || followsTerminal) && terminalCwd) return terminalCwd
  return sftpFallbackPath(choice)
}

/** Where the first listing goes when the start path cannot be listed. */
export function sftpFallbackPath(choice: SftpStartDirectory): string | null {
  return choice === "last" ? null : "."
}
