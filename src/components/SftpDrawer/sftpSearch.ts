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

const REGEX_SPECIAL_CHARS = /[\\^$+?.()|{}]/g
const GLOB_SPECIAL_CHARS = /[*?[]/

function escapeRegex(text: string) {
  return text.replace(REGEX_SPECIAL_CHARS, "\\$&")
}

function escapeCharacterClassContent(text: string) {
  return text.replace(/\\/g, "\\\\").replace(/]/g, "\\]")
}

function globToRegexSource(glob: string) {
  let source = "^"

  for (let index = 0; index < glob.length; index += 1) {
    const char = glob[index]

    if (char === "*") {
      source += ".*"
      continue
    }

    if (char === "?") {
      source += "."
      continue
    }

    if (char === "[") {
      const closingIndex = glob.indexOf("]", index + 1)
      if (closingIndex === -1) {
        source += "\\["
        continue
      }

      const classContent = glob.slice(index + 1, closingIndex)
      if (!classContent) {
        source += "\\[\\]"
        index = closingIndex
        continue
      }

      const isNegated = classContent[0] === "!"
      const normalizedContent = isNegated ? classContent.slice(1) : classContent
      source += "[" + (isNegated ? "^" : "") + escapeCharacterClassContent(normalizedContent) + "]"
      index = closingIndex
      continue
    }

    source += escapeRegex(char)
  }

  return source + "$"
}

function createGlobPattern(glob: string) {
  return new RegExp(globToRegexSource(glob), "i")
}

function hasGlobSyntax(query: string) {
  return GLOB_SPECIAL_CHARS.test(query)
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
    if (hasGlobSyntax(query)) {
      const pattern = createGlobPattern(query)

      return {
        error: null,
        hasQuery: true,
        matches: (entry) => pattern.test(entry.name),
      }
    }

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
    if (hasGlobSyntax(query)) {
      const pattern = createGlobPattern(query)

      return {
        error: null,
        hasQuery: true,
        matches: (entry) => pattern.test(entry.name),
      }
    }

    return {
      error: error instanceof Error ? error : new Error(String(error)),
      hasQuery: true,
      matches: () => false,
    }
  }
}
