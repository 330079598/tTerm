/**
 * Back/forward history of the remote directories one SFTP drawer has shown.
 * Pure state transitions, so the navigation rules stay unit-testable.
 */

export const SFTP_HISTORY_LIMIT = 50

export interface SftpHistory {
  paths: string[]
  /** Position of the directory on screen; -1 before the first listing. */
  index: number
}

export const EMPTY_SFTP_HISTORY: SftpHistory = { paths: [], index: -1 }

export type SftpHistoryStep = -1 | 1

/**
 * Records a regular navigation. Reloading the current directory changes
 * nothing; a new directory drops the forward entries, like a browser.
 */
export function recordSftpVisit(history: SftpHistory, path: string): SftpHistory {
  if (history.paths[history.index] === path) {
    return history
  }

  const paths = [...history.paths.slice(0, history.index + 1), path].slice(-SFTP_HISTORY_LIMIT)
  return { paths, index: paths.length - 1 }
}

/** The index one step back or forward, or null at either end. */
export function sftpHistoryTarget(history: SftpHistory, step: SftpHistoryStep): number | null {
  const target = history.index + step
  return target >= 0 && target < history.paths.length ? target : null
}

/**
 * Moves onto `index` once its listing loaded. The server reports the
 * resolved path, which can differ from the stored one, so the entry follows it.
 */
export function landSftpHistory(history: SftpHistory, index: number, path: string): SftpHistory {
  if (index < 0 || index >= history.paths.length) {
    return recordSftpVisit(history, path)
  }
  if (history.index === index && history.paths[index] === path) {
    return history
  }

  const paths = history.paths[index] === path ? history.paths : [...history.paths]
  paths[index] = path
  return { paths, index }
}

/**
 * Forgets an entry whose directory failed to load (deleted, no longer
 * readable), so the next step skips past it instead of failing again.
 */
export function dropSftpHistoryEntry(history: SftpHistory, index: number): SftpHistory {
  if (index < 0 || index >= history.paths.length || index === history.index) {
    return history
  }

  return {
    paths: history.paths.filter((_, position) => position !== index),
    index: index < history.index ? history.index - 1 : history.index,
  }
}
