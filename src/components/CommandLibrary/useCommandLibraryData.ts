import type { TFunction } from "i18next"
import { invoke } from "@tauri-apps/api/core"
import { useCallback, useDeferredValue, useEffect, useMemo, useRef, useState } from "react"

import { filterSavedCommands, type ListMode } from "@/components/CommandLibrary/commandLibraryUtils"
import type { useToast } from "@/hooks/use-toast"
import type { useConfirmDialog } from "@/components/ui/app-dialog"
import { activeVariables } from "@/lib/commandVariables"
import type { RecentCommand, SavedCommand } from "@/types/command"
import { createCommandDraft } from "@/lib/recentCommands"
import { onSyncApplied } from "@/lib/sync"

interface CommandLibraryDataOptions {
  open: boolean
  initialSearchQuery: string
  recentCommands: RecentCommand[]
  onOpenChange: (open: boolean) => void
  confirm: ReturnType<typeof useConfirmDialog>["confirm"]
  toast: ReturnType<typeof useToast>["toast"]
  t: TFunction
}

/**
 * The saved commands and tags: loading them when the library opens, the
 * search and filter, the selection, and favoriting and deleting.
 */
export function useCommandLibraryData({
  open,
  initialSearchQuery,
  recentCommands,
  onOpenChange,
  confirm,
  toast,
  t,
}: CommandLibraryDataOptions) {
  const searchInputRef = useRef<HTMLInputElement>(null)
  const [commands, setCommands] = useState<SavedCommand[]>([])
  const [rawSelectedId, setSelectedId] = useState<string | null>(null)
  const [searchQuery, setSearchQuery] = useState("")
  const deferredQuery = useDeferredValue(searchQuery)
  const [mode, setMode] = useState<ListMode>("all")
  const [isLoading, setIsLoading] = useState(false)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [favoritePendingId, setFavoritePendingId] = useState<string | null>(null)
  const [mobileDetailOpen, setMobileDetailOpen] = useState(false)
  const [rawSelectedRecentId, setSelectedRecentId] = useState<string | null>(null)
  const [recentPendingId, setRecentPendingId] = useState<string | null>(null)
  const [tags, setTags] = useState<string[]>([])

  const fetchCommands = useCallback(async () => {
    try {
      const [loaded, loadedTags] = await Promise.all([
        invoke<SavedCommand[]>("list_saved_commands"),
        invoke<string[]>("list_command_tags"),
      ])
      setCommands(loaded)
      setTags(loadedTags)
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
  }, [])

  const loadCommands = useCallback(async () => {
    setIsLoading(true)
    setLoadError(null)
    await fetchCommands()
  }, [fetchCommands])

  useEffect(() => {
    if (!open) return
    return onSyncApplied(["commands"], () => void loadCommands())
  }, [loadCommands, open])

  const [openedWith, setOpenedWith] = useState({ open: false, query: "" })
  if (openedWith.open !== open || openedWith.query !== initialSearchQuery) {
    setOpenedWith({ open, query: initialSearchQuery })
    if (open) {
      setSearchQuery(initialSearchQuery)
      if (initialSearchQuery) setMode("all")
      setIsLoading(true)
      setLoadError(null)
    }
  }

  useEffect(() => {
    if (!open) return
    // eslint-disable-next-line react-hooks/set-state-in-effect -- state is only set after await; false positive fixed upstream in facebook/react#36734
    void fetchCommands()
    requestAnimationFrame(() => searchInputRef.current?.focus())
  }, [fetchCommands, initialSearchQuery, open])

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
  const availableTags = tags
  // Fall back to the first visible item while the chosen one is filtered out.
  const selectedId =
    mode === "recent" ||
    (rawSelectedId && filteredCommands.some((command) => command.id === rawSelectedId))
      ? rawSelectedId
      : (filteredCommands[0]?.id ?? null)
  const selectedRecentId =
    mode !== "recent" ||
    (rawSelectedRecentId &&
      filteredRecentCommands.some((command) => command.id === rawSelectedRecentId))
      ? rawSelectedRecentId
      : (filteredRecentCommands[0]?.id ?? null)
  const selectedCommand = commands.find((command) => command.id === selectedId) ?? null
  const selectedRecent = recentCommands.find((command) => command.id === selectedRecentId) ?? null
  const selectedVariables = useMemo(
    () => (selectedCommand ? activeVariables(selectedCommand) : []),
    [selectedCommand]
  )
  const hasSelectedItem = mode === "recent" ? Boolean(selectedRecent) : Boolean(selectedCommand)

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

  const isFiltering = searchQuery.trim().length > 0 || mode === "favorites"

  return {
    searchInputRef,
    commands,
    setCommands,
    selectedId,
    setSelectedId,
    searchQuery,
    setSearchQuery,
    mode,
    setMode,
    isLoading,
    loadError,
    favoritePendingId,
    mobileDetailOpen,
    setMobileDetailOpen,
    selectedRecentId,
    setSelectedRecentId,
    recentPendingId,
    tags,
    setTags,
    loadCommands,
    filteredCommands,
    filteredRecentCommands,
    availableTags,
    selectedCommand,
    selectedRecent,
    selectedVariables,
    hasSelectedItem,
    handleSaved,
    handleFavorite,
    handleRecentFavorite,
    handleDelete,
    isFiltering,
  }
}

export type CommandLibraryData = ReturnType<typeof useCommandLibraryData>
