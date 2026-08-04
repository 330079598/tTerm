import type { CommandDraft, ExecutedCommand, RecentCommand } from "@/types/command"

export const RECENT_COMMANDS_STORAGE_KEY = "tterm.recent-commands.v1"
export const RECENT_COMMANDS_LIMIT = 100

function commandKey(commandText: string, profileId?: string) {
  return `${profileId ?? "global"}\u0000${commandText.trim()}`
}

export function suggestCommandName(commandText: string): string {
  const firstLine = commandText
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find(Boolean)
  if (!firstLine) return ""
  return firstLine.length <= 48 ? firstLine : `${firstLine.slice(0, 45).trimEnd()}...`
}

export function createCommandDraft(
  commandText: string,
  profile?: { id: string; name: string }
): CommandDraft {
  const normalized = commandText.trim()
  return {
    name: suggestCommandName(normalized),
    commandText: normalized,
    description: "",
    tags: [],
    scopeType: profile ? "profile" : "global",
    scopeId: profile?.id,
    isFavorite: true,
  }
}

export function addRecentCommand(
  commands: RecentCommand[],
  executed: ExecutedCommand
): RecentCommand[] {
  const commandText = executed.commandText.trim()
  if (!commandText) return commands

  const key = commandKey(commandText, executed.profileId)
  const existing = commands.find(
    (command) => commandKey(command.commandText, command.profileId) === key
  )
  const recent: RecentCommand = {
    id: existing?.id ?? `${executed.executedAt}:${key}`,
    commandText,
    profileId: executed.profileId,
    profileName: executed.profileName,
    lastUsedAt: executed.executedAt,
    useCount: (existing?.useCount ?? 0) + 1,
  }

  return [recent, ...commands.filter((command) => command.id !== existing?.id)].slice(
    0,
    RECENT_COMMANDS_LIMIT
  )
}

export function loadRecentCommands(storage: Pick<Storage, "getItem">): RecentCommand[] {
  try {
    const value = storage.getItem(RECENT_COMMANDS_STORAGE_KEY)
    if (!value) return []
    const parsed = JSON.parse(value)
    if (!Array.isArray(parsed)) return []
    return parsed
      .filter(
        (item): item is RecentCommand =>
          item !== null &&
          typeof item === "object" &&
          typeof item.id === "string" &&
          typeof item.commandText === "string" &&
          typeof item.lastUsedAt === "number" &&
          typeof item.useCount === "number"
      )
      .slice(0, RECENT_COMMANDS_LIMIT)
  } catch {
    return []
  }
}

export function saveRecentCommands(storage: Pick<Storage, "setItem">, commands: RecentCommand[]) {
  storage.setItem(RECENT_COMMANDS_STORAGE_KEY, JSON.stringify(commands))
}
