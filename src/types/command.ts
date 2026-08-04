export interface SavedCommandVariable {
  name: string
  label: string
  valueType: "text" | "number" | "choice" | "secret"
  defaultValue?: string
  optionsJson?: string
  isRequired: boolean
  position: number
}

export interface SavedCommand {
  id: string
  name: string
  commandText: string
  description: string
  scopeType: "global" | "profile"
  scopeId?: string
  shellType: string
  platform: "any" | "windows" | "linux" | "macos"
  isFavorite: boolean
  confirmBeforeRun: boolean
  sortOrder: number
  useCount: number
  lastUsedAt?: number
  createdAt: number
  updatedAt: number
  tags: string[]
  variables: SavedCommandVariable[]
}

export interface SaveCommandInput {
  id?: string
  name: string
  commandText: string
  description: string
  tags: string[]
  scopeType: "global" | "profile"
  scopeId?: string
  isFavorite: boolean
}

export type CommandDraft = Omit<SaveCommandInput, "id">

export interface RecentCommand {
  id: string
  commandText: string
  profileId?: string
  profileName?: string
  lastUsedAt: number
  useCount: number
}

export interface ExecutedCommand {
  commandText: string
  profileId?: string
  profileName?: string
  executedAt: number
}
