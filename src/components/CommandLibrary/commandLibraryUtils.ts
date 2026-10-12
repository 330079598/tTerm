import type { SavedCommand } from "@/types/command"

export interface ActiveProfile {
  id: string
  name: string
}

export type ListMode = "all" | "favorites" | "recent"
export function parseCommandTags(value: string): string[] {
  const seen = new Set<string>()
  return value
    .split(/[,，\n]/)
    .map((tag) => tag.trim())
    .filter(Boolean)
    .filter((tag) => {
      const normalized = tag.toLocaleLowerCase()
      if (seen.has(normalized)) return false
      seen.add(normalized)
      return true
    })
}

export function addCommandTags(tags: string[], value: string): string[] {
  const nextTags = [...tags]
  const seen = new Set(tags.map((tag) => tag.toLocaleLowerCase()))

  for (const tag of parseCommandTags(value)) {
    const normalized = tag.toLocaleLowerCase()
    if (seen.has(normalized)) continue
    seen.add(normalized)
    nextTags.push(tag)
  }

  return nextTags
}

export function removeCommandTag(tags: string[], value: string): string[] {
  const normalizedValue = value.toLocaleLowerCase()
  return tags.filter((tag) => tag.toLocaleLowerCase() !== normalizedValue)
}

export function collectCommandTags(commands: SavedCommand[]): string[] {
  const tags = commands.reduce<string[]>(
    (current, command) => addCommandTags(current, command.tags.join("\n")),
    []
  )
  return tags.sort((left, right) => left.localeCompare(right, undefined, { sensitivity: "base" }))
}

export function filterSavedCommands(
  commands: SavedCommand[],
  query: string,
  mode: Exclude<ListMode, "recent">
): SavedCommand[] {
  const normalizedQuery = query.trim().toLocaleLowerCase()
  return commands.filter((command) => {
    if (mode === "favorites" && !command.isFavorite) return false
    if (!normalizedQuery) return true
    return [command.name, command.commandText, command.description, ...command.tags]
      .join("\n")
      .toLocaleLowerCase()
      .includes(normalizedQuery)
  })
}
