import type { SftpDirectoryEntry } from "@/components/SftpDrawer/types"

export type SftpSearchOptions = {
  regex: boolean
}

export type SftpSearchMatcher = {
  error: Error | null
  hasQuery: boolean
  matches: (entry: SftpDirectoryEntry) => boolean
}

export const DEFAULT_SFTP_SEARCH_OPTIONS: SftpSearchOptions = {
  regex: false,
}

export function createSftpSearchMatcher(
  searchQuery: string,
  options: SftpSearchOptions
): SftpSearchMatcher {
  const query = searchQuery.trim()

  if (!query) {
    return {
      error: null,
      hasQuery: false,
      matches: () => true,
    }
  }

  if (!options.regex) {
    const normalizedQuery = query.toLocaleLowerCase()

    return {
      error: null,
      hasQuery: true,
      matches: (entry) => entry.name.toLocaleLowerCase().includes(normalizedQuery),
    }
  }

  try {
    const pattern = new RegExp(query, "i")

    return {
      error: null,
      hasQuery: true,
      matches: (entry) => pattern.test(entry.name),
    }
  } catch (error) {
    return {
      error: error instanceof Error ? error : new Error(String(error)),
      hasQuery: true,
      matches: () => false,
    }
  }
}
