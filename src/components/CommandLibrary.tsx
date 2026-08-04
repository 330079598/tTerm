import { invoke } from "@tauri-apps/api/core"
import {
  AlertCircle,
  ArrowLeft,
  Braces,
  Check,
  Clock3,
  FileCode2,
  Library,
  Loader2,
  Pencil,
  Plus,
  Search,
  Star,
  Trash2,
  X,
} from "lucide-react"
import React, { useDeferredValue, useEffect, useMemo, useRef, useState } from "react"
import { useTranslation } from "react-i18next"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Select } from "@/components/ui/select"
import { Textarea } from "@/components/ui/textarea"
import { useConfirmDialog } from "@/components/ui/app-dialog"
import { useToast } from "@/hooks/use-toast"
import { cn } from "@/lib/utils"
import type { CommandDraft, RecentCommand, SaveCommandInput, SavedCommand } from "@/types/command"
import { createCommandDraft } from "@/lib/recentCommands"

interface ActiveProfile {
  id: string
  name: string
}

interface CommandLibraryProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  activeProfile?: ActiveProfile
  canInsert: boolean
  onInsert: (command: SavedCommand) => Promise<boolean>
  onInsertRecent: (commandText: string) => Promise<boolean>
  recentCommands: RecentCommand[]
  initialSearchQuery?: string
}

type ListMode = "all" | "favorites" | "recent"

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

export function CommandEditorDialog({
  open,
  command,
  draft,
  availableTags,
  activeProfile,
  onOpenChange,
  onSaved,
}: {
  open: boolean
  command: SavedCommand | null
  draft?: CommandDraft | null
  availableTags: string[]
  activeProfile?: ActiveProfile
  onOpenChange: (open: boolean) => void
  onSaved: (command: SavedCommand) => void
}) {
  const { t } = useTranslation()
  const { toast } = useToast()
  const nameInputRef = useRef<HTMLInputElement>(null)
  const [name, setName] = useState("")
  const [commandText, setCommandText] = useState("")
  const [description, setDescription] = useState("")
  const [selectedTags, setSelectedTags] = useState<string[]>([])
  const [tagDraft, setTagDraft] = useState("")
  const [scopeType, setScopeType] = useState<"global" | "profile">("global")
  const [isFavorite, setIsFavorite] = useState(false)
  const [isSaving, setIsSaving] = useState(false)
  const [errors, setErrors] = useState<{ name?: string; commandText?: string }>({})
  const source = command ?? draft
  const profileScopeId = source?.scopeId ?? activeProfile?.id
  const profileScopeLabel =
    command?.scopeType === "profile" && command.scopeId !== activeProfile?.id
      ? t("commandLibrary.form.savedConnection")
      : activeProfile?.name

  useEffect(() => {
    if (!open) return
    setName(source?.name ?? "")
    setCommandText(source?.commandText ?? "")
    setDescription(source?.description ?? "")
    setSelectedTags(parseCommandTags(source?.tags.join("\n") ?? ""))
    setTagDraft("")
    setScopeType(source?.scopeType ?? "global")
    setIsFavorite(source?.isFavorite ?? false)
    setErrors({})
    setIsSaving(false)
    requestAnimationFrame(() => nameInputRef.current?.focus())
  }, [open, source])

  const validate = () => {
    const nextErrors: typeof errors = {}
    if (!name.trim()) nextErrors.name = t("commandLibrary.form.nameRequired")
    if (!commandText.trim()) nextErrors.commandText = t("commandLibrary.form.commandRequired")
    setErrors(nextErrors)
    if (nextErrors.name) nameInputRef.current?.focus()
    return Object.keys(nextErrors).length === 0
  }

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault()
    if (!validate()) return

    const input: SaveCommandInput = {
      id: command?.id,
      name,
      commandText,
      description,
      tags: addCommandTags(selectedTags, tagDraft),
      scopeType,
      scopeId: scopeType === "profile" ? profileScopeId : undefined,
      isFavorite,
    }

    setIsSaving(true)
    try {
      const saved = await invoke<SavedCommand>("save_saved_command", { input })
      onSaved(saved)
      onOpenChange(false)
      toast({
        title: command
          ? t("commandLibrary.messages.updated")
          : t("commandLibrary.messages.created"),
      })
    } catch (error) {
      toast({
        title: t("commandLibrary.messages.saveFailed"),
        description: String(error),
        variant: "destructive",
      })
    } finally {
      setIsSaving(false)
    }
  }

  const remainingTags = availableTags.filter(
    (availableTag) =>
      !selectedTags.some(
        (selectedTag) => selectedTag.toLocaleLowerCase() === availableTag.toLocaleLowerCase()
      )
  )

  const commitTagDraft = () => {
    if (!tagDraft.trim()) return
    setSelectedTags((current) => addCommandTags(current, tagDraft))
    setTagDraft("")
  }

  return (
    <Dialog open={open} onOpenChange={(nextOpen) => !isSaving && onOpenChange(nextOpen)}>
      <DialogContent className="max-h-[88vh] overflow-y-auto sm:max-w-xl">
        <form onSubmit={handleSubmit} className="space-y-5">
          <DialogHeader>
            <DialogTitle>
              {command ? t("commandLibrary.editTitle") : t("commandLibrary.newTitle")}
            </DialogTitle>
            <DialogDescription>{t("commandLibrary.form.description")}</DialogDescription>
          </DialogHeader>

          <div className="space-y-2">
            <Label htmlFor="saved-command-name">{t("commandLibrary.form.name")}</Label>
            <Input
              ref={nameInputRef}
              id="saved-command-name"
              value={name}
              maxLength={120}
              aria-invalid={Boolean(errors.name)}
              aria-describedby={errors.name ? "saved-command-name-error" : undefined}
              onBlur={() => {
                if (!name.trim()) {
                  setErrors((current) => ({
                    ...current,
                    name: t("commandLibrary.form.nameRequired"),
                  }))
                }
              }}
              onChange={(event) => {
                setName(event.target.value)
                if (errors.name) setErrors((current) => ({ ...current, name: undefined }))
              }}
            />
            {errors.name && (
              <p id="saved-command-name-error" role="alert" className="text-destructive text-xs">
                {errors.name}
              </p>
            )}
          </div>

          <div className="space-y-2">
            <Label htmlFor="saved-command-text">{t("commandLibrary.form.command")}</Label>
            <Textarea
              id="saved-command-text"
              value={commandText}
              maxLength={65_536}
              className="min-h-36 font-mono leading-6"
              aria-invalid={Boolean(errors.commandText)}
              aria-describedby={
                errors.commandText ? "saved-command-text-error" : "saved-command-text-hint"
              }
              onBlur={() => {
                if (!commandText.trim()) {
                  setErrors((current) => ({
                    ...current,
                    commandText: t("commandLibrary.form.commandRequired"),
                  }))
                }
              }}
              onChange={(event) => {
                setCommandText(event.target.value)
                if (errors.commandText) {
                  setErrors((current) => ({ ...current, commandText: undefined }))
                }
              }}
            />
            {errors.commandText ? (
              <p id="saved-command-text-error" role="alert" className="text-destructive text-xs">
                {errors.commandText}
              </p>
            ) : (
              <p id="saved-command-text-hint" className="text-muted-foreground text-xs">
                {t("commandLibrary.form.commandHint")}
              </p>
            )}
          </div>

          <div className="space-y-2">
            <Label htmlFor="saved-command-description">
              {t("commandLibrary.form.descriptionLabel")}
            </Label>
            <Textarea
              id="saved-command-description"
              value={description}
              className="min-h-20"
              onChange={(event) => setDescription(event.target.value)}
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="saved-command-tags">{t("commandLibrary.form.tags")}</Label>
            {selectedTags.length > 0 && (
              <div
                className="border-input bg-muted/20 flex min-h-11 flex-wrap items-center gap-2 rounded-md border px-2 py-2"
                aria-label={t("commandLibrary.form.selectedTags")}
                role="list"
              >
                {selectedTags.map((tag) => (
                  <span
                    key={tag.toLocaleLowerCase()}
                    className="bg-secondary text-secondary-foreground inline-flex min-h-7 max-w-full items-center gap-1 rounded-md border px-2 text-xs font-medium"
                    role="listitem"
                  >
                    <span className="truncate">{tag}</span>
                    <button
                      type="button"
                      className="text-muted-foreground hover:bg-accent hover:text-foreground focus-visible:ring-ring/50 -mr-1 flex size-6 shrink-0 items-center justify-center rounded-sm transition-colors outline-none focus-visible:ring-[3px]"
                      aria-label={t("commandLibrary.form.removeTag", { tag })}
                      onClick={() => setSelectedTags((current) => removeCommandTag(current, tag))}
                    >
                      <X className="size-3.5" />
                    </button>
                  </span>
                ))}
              </div>
            )}
            <div className="flex items-stretch gap-2">
              <Input
                id="saved-command-tags"
                value={tagDraft}
                maxLength={64}
                className="h-11"
                placeholder={t("commandLibrary.form.tagsPlaceholder")}
                aria-describedby="saved-command-tags-hint"
                onKeyDown={(event) => {
                  if (event.key !== "Enter" && event.key !== "," && event.key !== "，") return
                  event.preventDefault()
                  commitTagDraft()
                }}
                onChange={(event) => {
                  const value = event.target.value
                  if (/[,，\n]/.test(value)) {
                    setSelectedTags((current) => addCommandTags(current, value))
                    setTagDraft("")
                    return
                  }
                  setTagDraft(value)
                }}
              />
              <Button
                type="button"
                variant="outline"
                className="h-11 shrink-0"
                disabled={!tagDraft.trim()}
                onClick={commitTagDraft}
              >
                <Plus />
                {t("commandLibrary.form.addTag")}
              </Button>
            </div>
            {remainingTags.length > 0 && (
              <div className="space-y-2 pt-1">
                <p className="text-muted-foreground text-xs font-medium">
                  {t("commandLibrary.form.availableTags")}
                </p>
                <div className="flex max-h-28 flex-wrap gap-2 overflow-y-auto pr-1">
                  {remainingTags.map((tag) => (
                    <button
                      key={tag.toLocaleLowerCase()}
                      type="button"
                      className="border-input bg-background text-muted-foreground hover:bg-accent hover:text-accent-foreground focus-visible:ring-ring/50 inline-flex min-h-8 max-w-full items-center gap-1.5 rounded-md border px-2.5 text-xs font-medium transition-colors outline-none focus-visible:ring-[3px]"
                      onClick={() => setSelectedTags((current) => addCommandTags(current, tag))}
                    >
                      <Plus className="size-3.5 shrink-0" />
                      <span className="truncate">{tag}</span>
                    </button>
                  ))}
                </div>
              </div>
            )}
            <p id="saved-command-tags-hint" className="text-muted-foreground text-xs">
              {t("commandLibrary.form.tagsHint")}
            </p>
          </div>

          {profileScopeId && (
            <div className="space-y-2">
              <Label htmlFor="saved-command-scope">{t("commandLibrary.form.scope")}</Label>
              <Select
                id="saved-command-scope"
                value={scopeType}
                onChange={(event) => setScopeType(event.target.value as "global" | "profile")}
              >
                <option value="global">{t("commandLibrary.scope.global")}</option>
                <option value="profile">
                  {t("commandLibrary.scope.connection", { name: profileScopeLabel })}
                </option>
              </Select>
            </div>
          )}

          <label className="hover:bg-muted/40 flex min-h-11 cursor-pointer items-center gap-3 rounded-md border px-3 py-2 transition-colors">
            <Checkbox
              checked={isFavorite}
              onCheckedChange={setIsFavorite}
              aria-label={t("commandLibrary.form.favorite")}
            />
            <span className="flex min-w-0 flex-col gap-1">
              <span className="text-sm font-medium">{t("commandLibrary.form.favorite")}</span>
              <span className="text-muted-foreground text-xs">
                {t("commandLibrary.form.favoriteHint")}
              </span>
            </span>
          </label>

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              disabled={isSaving}
              onClick={() => onOpenChange(false)}
            >
              {t("common.cancel")}
            </Button>
            <Button type="submit" disabled={isSaving}>
              {isSaving ? <Loader2 className="animate-spin" /> : <Check />}
              {isSaving ? t("common.saving") : t("common.save")}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

export const CommandLibrary: React.FC<CommandLibraryProps> = ({
  open,
  onOpenChange,
  activeProfile,
  canInsert,
  onInsert,
  onInsertRecent,
  recentCommands,
  initialSearchQuery = "",
}) => {
  const { t } = useTranslation()
  const { toast } = useToast()
  const { confirm, ConfirmDialog } = useConfirmDialog()
  const searchInputRef = useRef<HTMLInputElement>(null)
  const [commands, setCommands] = useState<SavedCommand[]>([])
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [searchQuery, setSearchQuery] = useState("")
  const deferredQuery = useDeferredValue(searchQuery)
  const [mode, setMode] = useState<ListMode>("all")
  const [isLoading, setIsLoading] = useState(false)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [editorOpen, setEditorOpen] = useState(false)
  const [editingCommand, setEditingCommand] = useState<SavedCommand | null>(null)
  const [favoritePendingId, setFavoritePendingId] = useState<string | null>(null)
  const [isInserting, setIsInserting] = useState(false)
  const [mobileDetailOpen, setMobileDetailOpen] = useState(false)
  const [selectedRecentId, setSelectedRecentId] = useState<string | null>(null)
  const [recentPendingId, setRecentPendingId] = useState<string | null>(null)

  const loadCommands = async () => {
    setIsLoading(true)
    setLoadError(null)
    try {
      const loaded = await invoke<SavedCommand[]>("list_saved_commands")
      setCommands(loaded)
      setMobileDetailOpen(false)
      setSelectedId((current) =>
        current && loaded.some((command) => command.id === current)
          ? current
          : (loaded[0]?.id ?? null)
      )
    } catch (error) {
      setLoadError(String(error))
    } finally {
      setIsLoading(false)
    }
  }

  useEffect(() => {
    if (!open) return
    setSearchQuery(initialSearchQuery)
    if (initialSearchQuery) setMode("all")
    void loadCommands()
    requestAnimationFrame(() => searchInputRef.current?.focus())
  }, [initialSearchQuery, open])

  const filteredCommands = useMemo(
    () => (mode === "recent" ? [] : filterSavedCommands(commands, deferredQuery, mode)),
    [commands, deferredQuery, mode]
  )
  const filteredRecentCommands = useMemo(() => {
    const query = deferredQuery.trim().toLocaleLowerCase()
    return recentCommands.filter((command) =>
      query
        ? [command.commandText, command.profileName ?? ""]
            .join("\n")
            .toLocaleLowerCase()
            .includes(query)
        : true
    )
  }, [deferredQuery, recentCommands])
  const availableTags = useMemo(() => collectCommandTags(commands), [commands])
  const selectedCommand = commands.find((command) => command.id === selectedId) ?? null
  const selectedRecent = recentCommands.find((command) => command.id === selectedRecentId) ?? null
  const hasSelectedItem = mode === "recent" ? Boolean(selectedRecent) : Boolean(selectedCommand)

  useEffect(() => {
    if (mode === "recent") return
    if (selectedId && filteredCommands.some((command) => command.id === selectedId)) return
    setSelectedId(filteredCommands[0]?.id ?? null)
  }, [filteredCommands, mode, selectedId])

  useEffect(() => {
    if (mode !== "recent") return
    if (
      selectedRecentId &&
      filteredRecentCommands.some((command) => command.id === selectedRecentId)
    ) {
      return
    }
    setSelectedRecentId(filteredRecentCommands[0]?.id ?? null)
  }, [filteredRecentCommands, mode, selectedRecentId])

  const handleSaved = (saved: SavedCommand) => {
    setCommands((current) => {
      const exists = current.some((command) => command.id === saved.id)
      const next = exists
        ? current.map((command) => (command.id === saved.id ? saved : command))
        : [saved, ...current]
      return next.sort(
        (left, right) =>
          Number(right.isFavorite) - Number(left.isFavorite) || right.updatedAt - left.updatedAt
      )
    })
    setSelectedId(saved.id)
  }

  const handleFavorite = async (command: SavedCommand) => {
    if (favoritePendingId) return
    setFavoritePendingId(command.id)
    try {
      const saved = await invoke<SavedCommand>("set_saved_command_favorite", {
        id: command.id,
        favorite: !command.isFavorite,
      })
      handleSaved(saved)
    } catch (error) {
      toast({
        title: t("commandLibrary.messages.favoriteFailed"),
        description: String(error),
        variant: "destructive",
      })
    } finally {
      setFavoritePendingId(null)
    }
  }

  const handleRecentFavorite = async (recent: RecentCommand) => {
    if (recentPendingId) return
    const existing = commands.find(
      (command) =>
        command.commandText.trim() === recent.commandText.trim() &&
        command.scopeId === recent.profileId
    )
    setRecentPendingId(recent.id)
    try {
      const saved = existing
        ? await invoke<SavedCommand>("set_saved_command_favorite", {
            id: existing.id,
            favorite: !existing.isFavorite,
          })
        : await invoke<SavedCommand>("save_saved_command", {
            input: createCommandDraft(
              recent.commandText,
              recent.profileId && recent.profileName
                ? { id: recent.profileId, name: recent.profileName }
                : undefined
            ),
          })
      handleSaved(saved)
      toast({
        title: saved.isFavorite
          ? t("commandLibrary.messages.favoriteAdded")
          : t("commandLibrary.messages.favoriteRemoved"),
      })
    } catch (error) {
      toast({
        title: t("commandLibrary.messages.favoriteFailed"),
        description: String(error),
        variant: "destructive",
      })
    } finally {
      setRecentPendingId(null)
    }
  }

  const handleDelete = async (command: SavedCommand) => {
    onOpenChange(false)
    const confirmed = await confirm({
      title: t("commandLibrary.deleteTitle"),
      description: t("commandLibrary.deleteDescription", { name: command.name }),
      confirmText: t("commandLibrary.deleteAction"),
      cancelText: t("common.cancel"),
      defaultAction: "cancel",
      variant: "destructive",
    })
    if (!confirmed) {
      onOpenChange(true)
      return
    }

    try {
      await invoke("delete_saved_command", { id: command.id })
      setCommands((current) => current.filter((candidate) => candidate.id !== command.id))
      setSelectedId((current) => (current === command.id ? null : current))
      toast({ title: t("commandLibrary.messages.deleted") })
    } catch (error) {
      toast({
        title: t("commandLibrary.messages.deleteFailed"),
        description: String(error),
        variant: "destructive",
      })
    } finally {
      onOpenChange(true)
    }
  }

  const openEditor = (command: SavedCommand | null) => {
    setEditingCommand(command)
    onOpenChange(false)
    setEditorOpen(true)
  }

  const handleEditorOpenChange = (nextOpen: boolean) => {
    setEditorOpen(nextOpen)
    if (!nextOpen) onOpenChange(true)
  }

  const handleInsert = async (command: SavedCommand) => {
    setIsInserting(true)
    try {
      const inserted = await onInsert(command)
      if (!inserted) return
      void invoke("record_saved_command_use", { id: command.id }).catch(console.error)
      setCommands((current) =>
        current.map((candidate) =>
          candidate.id === command.id
            ? { ...candidate, useCount: candidate.useCount + 1, lastUsedAt: Date.now() }
            : candidate
        )
      )
      onOpenChange(false)
    } finally {
      setIsInserting(false)
    }
  }

  const isFiltering = searchQuery.trim().length > 0 || mode === "favorites"

  return (
    <>
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent
          showCloseButton={false}
          className="flex h-[min(720px,calc(100dvh-2rem))] w-[min(960px,calc(100vw-2rem))] max-w-none flex-col gap-0 overflow-hidden p-0 sm:max-w-none"
        >
          <header className="border-border/80 flex min-h-14 items-center justify-between gap-2 border-b px-3 py-2 sm:min-h-16 sm:gap-4 sm:px-5 sm:py-3">
            <div className="flex min-w-0 items-center gap-3">
              <div className="bg-muted text-muted-foreground hidden size-9 shrink-0 items-center justify-center rounded-md border sm:flex">
                <Library className="size-4" />
              </div>
              <div className="min-w-0">
                <DialogTitle className="text-base">{t("commandLibrary.title")}</DialogTitle>
                <DialogDescription className="mt-1 hidden truncate sm:block">
                  {t("commandLibrary.description")}
                </DialogDescription>
              </div>
            </div>
            <div className="flex shrink-0 items-center gap-2">
              <Button
                type="button"
                size="sm"
                aria-label={t("commandLibrary.newCommand")}
                onClick={() => {
                  openEditor(null)
                }}
              >
                <Plus />
                <span className="hidden sm:inline">{t("commandLibrary.newCommand")}</span>
              </Button>
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                aria-label={t("common.close")}
                onClick={() => onOpenChange(false)}
              >
                <X />
              </Button>
            </div>
          </header>

          <div className="grid min-h-0 flex-1 grid-cols-1 md:grid-cols-[minmax(280px,0.82fr)_minmax(0,1.18fr)]">
            <section
              className={cn(
                "border-border/80 min-h-0 flex-col border-b md:flex md:border-r md:border-b-0",
                mobileDetailOpen && hasSelectedItem ? "hidden" : "flex"
              )}
            >
              <div className="space-y-3 border-b p-4">
                <div className="relative">
                  <Search className="text-muted-foreground pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2" />
                  <Input
                    ref={searchInputRef}
                    type="search"
                    value={searchQuery}
                    placeholder={t("commandLibrary.searchPlaceholder")}
                    aria-label={t("commandLibrary.searchPlaceholder")}
                    className="pl-9"
                    onChange={(event) => setSearchQuery(event.target.value)}
                  />
                </div>
                <div
                  className="bg-muted/55 grid grid-cols-3 rounded-md p-1"
                  role="group"
                  aria-label={t("commandLibrary.filterLabel")}
                >
                  {(["all", "favorites", "recent"] as const).map((value) => (
                    <button
                      key={value}
                      type="button"
                      aria-pressed={mode === value}
                      onClick={() => setMode(value)}
                      className={cn(
                        "focus-visible:ring-ring/50 min-h-8 rounded-sm px-3 text-xs font-medium transition-colors outline-none focus-visible:ring-[3px]",
                        mode === value
                          ? "bg-background text-foreground shadow-xs"
                          : "text-muted-foreground hover:text-foreground"
                      )}
                    >
                      {t(`commandLibrary.filters.${value}`)}
                    </button>
                  ))}
                </div>
              </div>

              <ScrollArea className="min-h-0 flex-1">
                {isLoading && (
                  <div className="flex min-h-48 items-center justify-center" role="status">
                    <Loader2 className="text-muted-foreground size-5 animate-spin" />
                    <span className="sr-only">{t("commandLibrary.loading")}</span>
                  </div>
                )}

                {!isLoading && loadError && (
                  <div className="flex min-h-48 flex-col items-center justify-center px-6 text-center">
                    <AlertCircle className="text-destructive size-5" />
                    <p className="mt-3 text-sm font-medium">{t("commandLibrary.loadFailed")}</p>
                    <p className="text-muted-foreground mt-1 max-w-xs text-xs break-words">
                      {loadError}
                    </p>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      className="mt-4"
                      onClick={loadCommands}
                    >
                      {t("commandLibrary.retry")}
                    </Button>
                  </div>
                )}

                {!isLoading &&
                  !loadError &&
                  (mode === "recent"
                    ? filteredRecentCommands.length === 0
                    : filteredCommands.length === 0) && (
                    <div className="flex min-h-52 flex-col items-center justify-center px-6 text-center">
                      {mode === "recent" ? (
                        <Clock3 className="text-muted-foreground size-6" />
                      ) : (
                        <FileCode2 className="text-muted-foreground size-6" />
                      )}
                      <p className="mt-3 text-sm font-medium">
                        {mode === "recent"
                          ? t("commandLibrary.recentEmpty")
                          : isFiltering
                            ? t("commandLibrary.noResults")
                            : t("commandLibrary.empty")}
                      </p>
                      <p className="text-muted-foreground mt-1 max-w-xs text-xs leading-5">
                        {mode === "recent"
                          ? t("commandLibrary.recentEmptyHint")
                          : isFiltering
                            ? t("commandLibrary.noResultsHint")
                            : t("commandLibrary.emptyHint")}
                      </p>
                      {!isFiltering && mode !== "recent" && (
                        <Button
                          type="button"
                          variant="outline"
                          size="sm"
                          className="mt-4"
                          onClick={() => {
                            openEditor(null)
                          }}
                        >
                          <Plus />
                          {t("commandLibrary.newCommand")}
                        </Button>
                      )}
                    </div>
                  )}

                {!isLoading && !loadError && mode !== "recent" && filteredCommands.length > 0 && (
                  <div role="listbox" aria-label={t("commandLibrary.listLabel")} className="py-1">
                    {filteredCommands.map((command) => (
                      <div
                        key={command.id}
                        className={cn(
                          "group relative border-b last:border-b-0",
                          selectedId === command.id && "bg-muted/55"
                        )}
                      >
                        {selectedId === command.id && (
                          <span className="bg-primary absolute inset-y-2 left-0 w-0.5 rounded-r" />
                        )}
                        <button
                          type="button"
                          role="option"
                          aria-selected={selectedId === command.id}
                          className="focus-visible:ring-ring/50 w-full px-4 py-3 pr-12 text-left outline-none focus-visible:ring-[3px] focus-visible:ring-inset"
                          onClick={() => {
                            setSelectedId(command.id)
                            setMobileDetailOpen(true)
                          }}
                        >
                          <span className="block truncate text-sm font-medium">{command.name}</span>
                          <code className="text-muted-foreground mt-1.5 block truncate font-mono text-xs">
                            {command.commandText}
                          </code>
                          <span className="mt-2 flex min-h-5 flex-wrap items-center gap-1.5">
                            {command.scopeType === "profile" && (
                              <Badge
                                variant="outline"
                                className="rounded-md px-1.5 py-0 text-[10px]"
                              >
                                {t("commandLibrary.scope.profileBadge")}
                              </Badge>
                            )}
                            {command.tags.slice(0, 2).map((tag) => (
                              <Badge
                                key={tag.toLocaleLowerCase()}
                                variant="secondary"
                                className="max-w-24 rounded-md px-1.5 py-0 text-[10px]"
                              >
                                <span className="truncate">{tag}</span>
                              </Badge>
                            ))}
                          </span>
                        </button>
                        <button
                          type="button"
                          disabled={favoritePendingId === command.id}
                          aria-label={
                            command.isFavorite
                              ? t("commandLibrary.unfavorite")
                              : t("commandLibrary.favorite")
                          }
                          className="text-muted-foreground hover:bg-accent hover:text-foreground focus-visible:ring-ring/50 absolute top-2.5 right-2 flex size-8 items-center justify-center rounded-md transition-colors outline-none focus-visible:ring-[3px] disabled:opacity-50"
                          onClick={() => void handleFavorite(command)}
                        >
                          {favoritePendingId === command.id ? (
                            <Loader2 className="size-3.5 animate-spin" />
                          ) : (
                            <Star
                              className={cn(
                                "size-3.5",
                                command.isFavorite && "fill-current text-amber-500"
                              )}
                            />
                          )}
                        </button>
                      </div>
                    ))}
                  </div>
                )}

                {!isLoading &&
                  !loadError &&
                  mode === "recent" &&
                  filteredRecentCommands.length > 0 && (
                    <div
                      role="listbox"
                      aria-label={t("commandLibrary.recentListLabel")}
                      className="py-1"
                    >
                      {filteredRecentCommands.map((recent) => {
                        const saved = commands.find(
                          (command) =>
                            command.commandText.trim() === recent.commandText.trim() &&
                            command.scopeId === recent.profileId
                        )
                        return (
                          <div
                            key={recent.id}
                            className={cn(
                              "group relative border-b last:border-b-0",
                              selectedRecentId === recent.id && "bg-muted/55"
                            )}
                          >
                            {selectedRecentId === recent.id && (
                              <span className="bg-primary absolute inset-y-2 left-0 w-0.5 rounded-r" />
                            )}
                            <button
                              type="button"
                              role="option"
                              aria-selected={selectedRecentId === recent.id}
                              className="focus-visible:ring-ring/50 w-full px-4 py-3 pr-12 text-left outline-none focus-visible:ring-[3px] focus-visible:ring-inset"
                              onClick={() => {
                                setSelectedRecentId(recent.id)
                                setMobileDetailOpen(true)
                              }}
                            >
                              <code className="block truncate font-mono text-sm font-medium">
                                {recent.commandText}
                              </code>
                              <span className="text-muted-foreground mt-2 flex items-center gap-2 text-xs">
                                <span>
                                  {recent.profileName ?? t("commandLibrary.scope.global")}
                                </span>
                                <span aria-hidden="true">·</span>
                                <span>
                                  {t("commandLibrary.recentUseCount", { count: recent.useCount })}
                                </span>
                              </span>
                            </button>
                            <button
                              type="button"
                              disabled={recentPendingId === recent.id}
                              aria-label={
                                saved?.isFavorite
                                  ? t("commandLibrary.unfavorite")
                                  : t("commandLibrary.favorite")
                              }
                              className="text-muted-foreground hover:bg-accent hover:text-foreground focus-visible:ring-ring/50 absolute top-2.5 right-2 flex size-8 items-center justify-center rounded-md transition-colors outline-none focus-visible:ring-[3px] disabled:opacity-50"
                              onClick={() => void handleRecentFavorite(recent)}
                            >
                              {recentPendingId === recent.id ? (
                                <Loader2 className="size-3.5 animate-spin" />
                              ) : (
                                <Star
                                  className={cn(
                                    "size-3.5",
                                    saved?.isFavorite && "fill-current text-amber-500"
                                  )}
                                />
                              )}
                            </button>
                          </div>
                        )
                      })}
                    </div>
                  )}
              </ScrollArea>
            </section>

            <section
              className={cn(
                "bg-background min-h-0 overflow-auto md:block",
                mobileDetailOpen && hasSelectedItem ? "block" : "hidden"
              )}
            >
              {mode === "recent" && selectedRecent ? (
                <div className="flex min-h-full flex-col p-5">
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="mb-4 w-fit md:hidden"
                    onClick={() => setMobileDetailOpen(false)}
                  >
                    <ArrowLeft />
                    {t("commandLibrary.backToList")}
                  </Button>
                  <div className="flex items-start justify-between gap-4">
                    <div className="min-w-0">
                      <h3 className="text-base font-semibold">
                        {t("commandLibrary.recentCommand")}
                      </h3>
                      <p className="text-muted-foreground mt-2 text-sm">
                        {selectedRecent.profileName ?? t("commandLibrary.scope.global")}
                      </p>
                    </div>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-sm"
                      disabled={recentPendingId === selectedRecent.id}
                      aria-label={t("commandLibrary.favorite")}
                      onClick={() => void handleRecentFavorite(selectedRecent)}
                    >
                      {recentPendingId === selectedRecent.id ? (
                        <Loader2 className="animate-spin" />
                      ) : (
                        <Star />
                      )}
                    </Button>
                  </div>
                  <div className="mt-5 min-h-32 rounded-md border bg-[hsl(var(--muted)/0.35)] p-4">
                    <div className="text-muted-foreground mb-3 flex items-center gap-2 text-xs font-medium">
                      <Braces className="size-3.5" />
                      {t("commandLibrary.commandPreview")}
                    </div>
                    <pre className="text-foreground overflow-x-auto font-mono text-sm leading-6 break-all whitespace-pre-wrap">
                      {selectedRecent.commandText}
                    </pre>
                  </div>
                  <dl className="mt-5 grid grid-cols-2 gap-x-6 gap-y-4 text-sm">
                    <div>
                      <dt className="text-muted-foreground text-xs">
                        {t("commandLibrary.usedLabel")}
                      </dt>
                      <dd className="mt-1 font-medium tabular-nums">
                        {t("commandLibrary.recentUseCount", { count: selectedRecent.useCount })}
                      </dd>
                    </div>
                    <div>
                      <dt className="text-muted-foreground text-xs">
                        {t("commandLibrary.lastUsedLabel")}
                      </dt>
                      <dd className="mt-1 font-medium">
                        {new Intl.DateTimeFormat(undefined, {
                          dateStyle: "short",
                          timeStyle: "short",
                        }).format(selectedRecent.lastUsedAt)}
                      </dd>
                    </div>
                  </dl>
                  <div className="mt-auto pt-6">
                    <Button
                      type="button"
                      className="w-full sm:w-auto"
                      disabled={!canInsert || isInserting}
                      onClick={async () => {
                        setIsInserting(true)
                        try {
                          if (await onInsertRecent(selectedRecent.commandText)) onOpenChange(false)
                        } finally {
                          setIsInserting(false)
                        }
                      }}
                    >
                      {isInserting ? <Loader2 className="animate-spin" /> : <FileCode2 />}
                      {isInserting ? t("commandLibrary.inserting") : t("commandLibrary.insert")}
                    </Button>
                  </div>
                </div>
              ) : selectedCommand ? (
                <div className="flex min-h-full flex-col p-5">
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="mb-4 w-fit md:hidden"
                    onClick={() => setMobileDetailOpen(false)}
                  >
                    <ArrowLeft />
                    {t("commandLibrary.backToList")}
                  </Button>
                  <div className="flex items-start justify-between gap-4">
                    <div className="min-w-0">
                      <div className="flex flex-wrap items-center gap-2">
                        <h3 className="text-base font-semibold break-words">
                          {selectedCommand.name}
                        </h3>
                        {selectedCommand.isFavorite && (
                          <Star className="size-4 fill-current text-amber-500" aria-hidden="true" />
                        )}
                      </div>
                      {selectedCommand.description && (
                        <p className="text-muted-foreground mt-2 text-sm leading-6 break-words">
                          {selectedCommand.description}
                        </p>
                      )}
                    </div>
                    <div className="flex shrink-0 items-center gap-1">
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon-sm"
                        aria-label={t("commandLibrary.edit")}
                        onClick={() => {
                          openEditor(selectedCommand)
                        }}
                      >
                        <Pencil />
                      </Button>
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon-sm"
                        className="hover:text-destructive"
                        aria-label={t("commandLibrary.deleteAction")}
                        onClick={() => void handleDelete(selectedCommand)}
                      >
                        <Trash2 />
                      </Button>
                    </div>
                  </div>

                  <div className="mt-5 min-h-32 rounded-md border bg-[hsl(var(--muted)/0.35)] p-4">
                    <div className="text-muted-foreground mb-3 flex items-center gap-2 text-xs font-medium">
                      <Braces className="size-3.5" />
                      {t("commandLibrary.commandPreview")}
                    </div>
                    <pre className="text-foreground overflow-x-auto font-mono text-sm leading-6 break-all whitespace-pre-wrap">
                      {selectedCommand.commandText}
                    </pre>
                  </div>

                  <dl className="mt-5 grid grid-cols-2 gap-x-6 gap-y-4 text-sm">
                    <div>
                      <dt className="text-muted-foreground text-xs">
                        {t("commandLibrary.scopeLabel")}
                      </dt>
                      <dd className="mt-1 font-medium">
                        {selectedCommand.scopeType === "global"
                          ? t("commandLibrary.scope.global")
                          : t("commandLibrary.scope.profileBadge")}
                      </dd>
                    </div>
                    <div>
                      <dt className="text-muted-foreground text-xs">
                        {t("commandLibrary.usedLabel")}
                      </dt>
                      <dd className="mt-1 font-medium tabular-nums">
                        {t("commandLibrary.usedCount", { count: selectedCommand.useCount })}
                      </dd>
                    </div>
                  </dl>

                  {selectedCommand.tags.length > 0 && (
                    <div className="mt-5 flex flex-wrap gap-2">
                      {selectedCommand.tags.map((tag) => (
                        <Badge
                          key={tag.toLocaleLowerCase()}
                          variant="secondary"
                          className="rounded-md"
                        >
                          {tag}
                        </Badge>
                      ))}
                    </div>
                  )}

                  <div className="mt-auto pt-6">
                    <Button
                      type="button"
                      className="w-full sm:w-auto"
                      disabled={!canInsert || isInserting}
                      onClick={() => void handleInsert(selectedCommand)}
                    >
                      {isInserting ? <Loader2 className="animate-spin" /> : <FileCode2 />}
                      {isInserting ? t("commandLibrary.inserting") : t("commandLibrary.insert")}
                    </Button>
                    {!canInsert && (
                      <p className="text-muted-foreground mt-2 text-xs">
                        {t("commandLibrary.selectTerminalHint")}
                      </p>
                    )}
                  </div>
                </div>
              ) : (
                <div className="text-muted-foreground flex min-h-full flex-col items-center justify-center px-8 text-center">
                  <Library className="size-7" />
                  <p className="mt-3 text-sm">{t("commandLibrary.selectHint")}</p>
                </div>
              )}
            </section>
          </div>
        </DialogContent>
      </Dialog>

      <CommandEditorDialog
        open={editorOpen}
        command={editingCommand}
        availableTags={availableTags}
        activeProfile={activeProfile}
        onOpenChange={handleEditorOpenChange}
        onSaved={handleSaved}
      />
      <ConfirmDialog />
    </>
  )
}
