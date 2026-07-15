import React, { useEffect, useMemo, useRef, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { useTranslation } from "react-i18next"
import {
  ChevronDown,
  ChevronRight,
  Copy,
  FileInput,
  Loader2,
  Pencil,
  PlugZap,
  Plus,
  Search,
  Server,
  Terminal,
  Trash2,
  X,
} from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { Button } from "@/components/ui/button"
import { useToast } from "@/hooks/use-toast"
import { Input } from "@/components/ui/input"
import { ScrollArea } from "@/components/ui/scroll-area"
import { useConfirmDialog } from "@/components/ui/app-dialog"
import { SshConfigImportDialog } from "@/components/SshConfigImportDialog"
import { buildConnectionFromProfile } from "@/lib/profileConnections"
import { cn } from "@/lib/utils"
import { Tab, type ConnectionType, type SavedProfile } from "@/types/tab"

interface ProfilesPanelProps {
  onConnect: (connection: Omit<Tab, "id" | "isActive">) => void
  onEdit: (profile: SavedProfile) => void
  onDuplicate: (profile: SavedProfile) => void
  collapsedGroupKeys?: string[]
  refreshKey?: number
  onCreate?: () => void
  onClose?: () => void
  onCollapsedGroupKeysChange?: (groups: string[]) => void
  surface?: "panel" | "plain"
  className?: string
}

export type { SavedProfile }

const UNGROUPED_KEY = "__ungrouped__"

const connectionTypeIcons = {
  ssh: Server,
  terminal: Terminal,
} as const

const getGroupListId = (group: string) => `profiles-group-${encodeURIComponent(group)}`

const buildConnectionSubtitle = (
  profile: SavedProfile,
  t: (key: string, options?: Record<string, unknown>) => string
) => {
  if (profile.connection_type === "terminal") {
    return t("profiles.localTerminal")
  }

  if (!profile.host) {
    return t("profiles.connectionDetailsUnavailable")
  }

  return `${profile.username ? `${profile.username}@` : ""}${profile.host}${profile.port && profile.port !== 22 ? `:${profile.port}` : ""}`
}

const buildMetaItems = (
  profile: SavedProfile,
  t: (key: string, options?: Record<string, unknown>) => string
) => {
  if (profile.connection_type === "terminal") {
    return [t("profiles.localTerminal")]
  }

  const items = ["SSH"]

  if (profile.auth_method === "key") {
    items.push(t("profiles.authMethodKey"))
  } else if (profile.auth_method === "password") {
    items.push(t("profiles.authMethodPassword"))
  }

  if (profile.port && profile.port !== 22) {
    items.push(t("profiles.portDisplay", { port: profile.port }))
  }

  const jumpHosts = profile.jump_hosts ?? []

  const useJumpHost = profile.use_jump_host ?? jumpHosts.length > 0
  if (useJumpHost && jumpHosts.length === 1) {
    items.push(t("jumpHost.via", { host: jumpHosts[0].host }))
  } else if (useJumpHost && jumpHosts.length > 1) {
    items.push(t("jumpHost.viaCount", { count: jumpHosts.length }))
  }

  return items
}

const ProfileRow: React.FC<{
  profile: SavedProfile
  isActive: boolean
  isDeleting: boolean
  onConnect: (p: SavedProfile) => void
  onEdit: (p: SavedProfile) => void
  onDuplicate: (p: SavedProfile) => void
  onDelete: (id: string) => void
  onFocusRow: (id: string) => void
  rowRef: (node: HTMLDivElement | null) => void
  t: (key: string, options?: Record<string, unknown>) => string
}> = ({
  profile,
  isActive,
  isDeleting,
  onConnect,
  onEdit,
  onDuplicate,
  onDelete,
  onFocusRow,
  rowRef,
  t,
}) => {
  const connectionType = profile.connection_type as ConnectionType
  const Icon = connectionTypeIcons[connectionType] ?? Server
  const subtitle = buildConnectionSubtitle(profile, t)
  const metaItems = buildMetaItems(profile, t)
  const handleKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Enter") {
      event.preventDefault()
      onConnect(profile)
      return
    }

    if (event.key === " ") {
      event.preventDefault()
      onFocusRow(profile.id)
    }
  }

  return (
    <div
      ref={rowRef}
      role="option"
      tabIndex={0}
      aria-selected={isActive}
      onClick={() => onFocusRow(profile.id)}
      onDoubleClick={() => onConnect(profile)}
      onKeyDown={handleKeyDown}
      className={cn(
        "group focus-visible:ring-ring/50 relative flex w-full items-start gap-3 rounded-xl border px-3 py-3 text-left transition-colors outline-none focus-visible:ring-[3px]",
        isActive
          ? "border-border bg-muted/55 shadow-[inset_0_0_0_1px_hsl(var(--border)/0.45)]"
          : "border-border/60 hover:bg-muted/30"
      )}
    >
      {isActive && (
        <div className="bg-foreground/28 absolute inset-y-2 left-0 w-0.5 rounded-full" />
      )}
      <div className="bg-muted text-muted-foreground flex size-9 shrink-0 items-center justify-center rounded-lg border">
        <Icon size={16} />
      </div>

      <div className="min-w-0 flex-1">
        <div className="flex min-w-0 items-center gap-2">
          <div className="truncate text-sm font-medium tracking-[-0.01em]">{profile.name}</div>
          <Badge variant="outline" className="hidden shrink-0 sm:inline-flex">
            {profile.connection_type === "terminal" ? t("profiles.localBadge") : "SSH"}
          </Badge>
        </div>

        <div className="text-muted-foreground mt-1 truncate text-sm">{subtitle}</div>

        <div className="text-muted-foreground mt-2 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs">
          {metaItems.map((item, index) => (
            <React.Fragment key={`${profile.id}-${item}`}>
              {index > 0 && <span className="text-border">•</span>}
              <span>{item}</span>
            </React.Fragment>
          ))}
        </div>
      </div>

      <div
        className={cn(
          "flex shrink-0 items-center gap-1 self-center transition-opacity",
          isActive
            ? "opacity-100"
            : "opacity-0 group-focus-within:opacity-100 group-hover:opacity-100"
        )}
      >
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              aria-label={t("profiles.connect")}
              onClick={(event) => {
                event.stopPropagation()
                onConnect(profile)
              }}
            >
              <PlugZap size={13} />
            </Button>
          </TooltipTrigger>
          <TooltipContent>{t("profiles.connect")}</TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              aria-label={t("profiles.edit")}
              onClick={(event) => {
                event.stopPropagation()
                onEdit(profile)
              }}
            >
              <Pencil size={13} />
            </Button>
          </TooltipTrigger>
          <TooltipContent>{t("profiles.edit")}</TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              aria-label={t("profiles.copy")}
              onClick={(event) => {
                event.stopPropagation()
                onDuplicate(profile)
              }}
            >
              <Copy size={13} />
            </Button>
          </TooltipTrigger>
          <TooltipContent>{t("profiles.copy")}</TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              className="hover:text-destructive"
              disabled={isDeleting}
              aria-label={t("profiles.delete")}
              onClick={(event) => {
                event.stopPropagation()
                onDelete(profile.id)
              }}
            >
              <Trash2 size={13} />
            </Button>
          </TooltipTrigger>
          <TooltipContent>{t("profiles.delete")}</TooltipContent>
        </Tooltip>
        <ChevronRight size={16} className="text-muted-foreground ml-0.5" />
      </div>
    </div>
  )
}

export const ProfilesPanel: React.FC<ProfilesPanelProps> = ({
  onConnect,
  onEdit,
  onDuplicate,
  collapsedGroupKeys = [],
  refreshKey,
  onCreate,
  onClose,
  onCollapsedGroupKeysChange,
  surface = "panel",
  className,
}) => {
  const { t } = useTranslation()
  const { toast } = useToast()
  const searchInputRef = useRef<HTMLInputElement | null>(null)
  const rowRefs = useRef<Record<string, HTMLDivElement | null>>({})
  const [profiles, setProfiles] = useState<SavedProfile[]>([])
  const [searchQuery, setSearchQuery] = useState("")
  const [selectedProfileId, setSelectedProfileId] = useState<string | null>(null)
  const [importDialogOpen, setImportDialogOpen] = useState(false)
  const [isLoading, setIsLoading] = useState(true)
  const [deletingId, setDeletingId] = useState<string | null>(null)
  const { confirm, ConfirmDialog } = useConfirmDialog()
  const collapsedGroupKeySet = useMemo(() => new Set(collapsedGroupKeys), [collapsedGroupKeys])

  useEffect(() => {
    setIsLoading(true)
    invoke<SavedProfile[]>("list_profiles")
      .then((result) => {
        setProfiles(result)
      })
      .catch((error) => {
        console.error("Failed to load profiles:", error)
        toast({
          title: t("common.error", { defaultValue: "Error" }),
          description: t("profiles.loadFailed", {
            defaultValue: "Failed to load connection profiles.",
          }),
          variant: "destructive",
        })
      })
      .finally(() => {
        setIsLoading(false)
      })
  }, [refreshKey, t, toast])

  useEffect(() => {
    if (surface !== "plain") {
      return
    }

    const frame = requestAnimationFrame(() => {
      searchInputRef.current?.focus()
    })

    return () => cancelAnimationFrame(frame)
  }, [surface])

  const handleDelete = async (id: string) => {
    const profile = profiles.find((item) => item.id === id)
    const deletePrompt = profile
      ? `${t("profiles.deleteConfirm")}\n\n${profile.name}`
      : t("profiles.deleteConfirm")

    const confirmed = await confirm({
      title: t("profiles.delete"),
      description: deletePrompt,
      confirmText: t("profiles.delete"),
      cancelText: t("common.cancel"),
      variant: "destructive",
    })

    if (!confirmed) return

    setDeletingId(id)
    try {
      await invoke("delete_profile", { id })
      setProfiles((prev) => prev.filter((item) => item.id !== id))
    } catch (error) {
      console.error("Failed to delete profile:", error)
      toast({
        title: t("common.error", { defaultValue: "Error" }),
        description: t("profiles.deleteFailed", {
          defaultValue: "Failed to delete connection profile.",
        }),
        variant: "destructive",
      })
    } finally {
      setDeletingId(null)
    }
  }

  const handleConnect = (profile: SavedProfile) => {
    onConnect(buildConnectionFromProfile(profile))
  }

  const groupedProfiles = useMemo(() => {
    const query = searchQuery.trim().toLowerCase()
    const filteredProfiles = profiles.filter((profile) => {
      if (!query) return true

      const searchTarget = [
        profile.name,
        profile.group,
        profile.host,
        profile.username,
        profile.connection_type,
      ]
        .filter(Boolean)
        .join(" ")
        .toLowerCase()

      return searchTarget.includes(query)
    })

    const groups = new Map<string, SavedProfile[]>()

    for (const profile of filteredProfiles) {
      const key = profile.group === "" ? UNGROUPED_KEY : profile.group
      const existingItems = groups.get(key)
      if (existingItems) {
        existingItems.push(profile)
      } else {
        groups.set(key, [profile])
      }
    }

    return Array.from(groups.entries()).sort(([left], [right]) => {
      if (left === UNGROUPED_KEY) return 1
      if (right === UNGROUPED_KEY) return -1
      return left.localeCompare(right)
    })
  }, [profiles, searchQuery])

  const flatProfiles = useMemo(
    () =>
      groupedProfiles.flatMap(([group, items]) =>
        collapsedGroupKeySet.has(group) && !searchQuery.trim() ? [] : items
      ),
    [collapsedGroupKeySet, groupedProfiles, searchQuery]
  )

  const activeProfileId = useMemo(() => {
    if (flatProfiles.length === 0) {
      return null
    }

    if (selectedProfileId && flatProfiles.some((profile) => profile.id === selectedProfileId)) {
      return selectedProfileId
    }

    return flatProfiles[0].id
  }, [flatProfiles, selectedProfileId])

  useEffect(() => {
    if (!activeProfileId) {
      return
    }

    rowRefs.current[activeProfileId]?.scrollIntoView({ block: "nearest" })
  }, [activeProfileId])

  const hasProfiles = profiles.length > 0
  const hasFilteredResults = groupedProfiles.length > 0
  const isFiltering = searchQuery.trim().length > 0

  const toggleGroupCollapsed = (group: string) => {
    const next = new Set(collapsedGroupKeySet)
    if (next.has(group)) {
      next.delete(group)
    } else {
      next.add(group)
    }
    onCollapsedGroupKeysChange?.(Array.from(next))
  }

  return (
    <section
      className={cn(
        "flex h-full min-h-[360px] flex-col",
        surface === "panel" && "bg-card rounded-lg border shadow-sm",
        className
      )}
    >
      <div className="border-border/80 flex items-start justify-between gap-4 border-b px-5 py-4">
        <div className="min-w-0 pr-6">
          <h2 className="text-base font-semibold tracking-[-0.02em]">{t("profiles.title")}</h2>
          <p className="text-muted-foreground mt-1 text-sm">{t("profiles.description")}</p>
        </div>

        <div className="flex shrink-0 items-center gap-2">
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="shrink-0"
                onClick={() => setImportDialogOpen(true)}
              >
                <FileInput size={14} />
                {t("profiles.importSshConfig")}
              </Button>
            </TooltipTrigger>
            <TooltipContent>{t("profiles.importSshConfigTooltip")}</TooltipContent>
          </Tooltip>
          {onCreate && (
            <Button type="button" size="sm" className="shrink-0" onClick={onCreate}>
              <Plus size={14} />
              {t("profiles.newConnection")}
            </Button>
          )}
          {onClose && (
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              onClick={onClose}
              aria-label={t("common.close")}
            >
              <X size={16} />
            </Button>
          )}
        </div>
      </div>

      <div className="px-5 pt-4 pb-3">
        <div className="relative">
          <Search className="text-muted-foreground pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2" />
          <Input
            ref={searchInputRef}
            type="search"
            value={searchQuery}
            onChange={(event) => setSearchQuery(event.target.value)}
            placeholder={t("profiles.searchPlaceholder")}
            className="pl-9"
          />
        </div>
      </div>

      <ScrollArea className="flex-1 px-3 pb-3">
        {isLoading && (
          <div className="flex min-h-[260px] flex-col items-center justify-center">
            <Loader2 className="text-muted-foreground size-6 animate-spin" />
          </div>
        )}

        {!isLoading && !hasProfiles && (
          <div className="flex min-h-[260px] flex-col items-center justify-center rounded-xl border border-dashed px-6 text-center">
            <div className="text-sm font-medium">{t("profiles.empty")}</div>
            <p className="text-muted-foreground mt-2 max-w-sm text-sm">
              {t("profiles.emptyDescription")}
            </p>
            {onCreate && (
              <div className="mt-4 flex flex-wrap justify-center gap-2">
                <Button type="button" size="sm" onClick={onCreate}>
                  <Plus size={14} />
                  {t("profiles.newConnection")}
                </Button>
                <Button
                  type="button"
                  size="sm"
                  variant="outline"
                  onClick={() => setImportDialogOpen(true)}
                >
                  <FileInput size={14} />
                  {t("profiles.importSshConfig")}
                </Button>
              </div>
            )}
          </div>
        )}

        {hasProfiles && !hasFilteredResults && (
          <div className="flex min-h-[220px] flex-col items-center justify-center rounded-xl border border-dashed px-6 text-center">
            <div className="text-sm font-medium">{t("profiles.searchEmpty")}</div>
            <p className="text-muted-foreground mt-2 max-w-sm text-sm">
              {t("profiles.searchEmptyDescription")}
            </p>
          </div>
        )}

        {hasFilteredResults && (
          <div className="space-y-5 px-2 pb-2">
            {groupedProfiles.map(([group, items]) => {
              const isCollapsed = collapsedGroupKeySet.has(group) && !isFiltering
              const groupLabel = group === UNGROUPED_KEY ? t("profiles.ungrouped") : group
              const groupListId = getGroupListId(group)

              return (
                <section key={group} className="space-y-2">
                  <button
                    type="button"
                    className="text-muted-foreground hover:bg-muted/35 focus-visible:ring-ring/50 flex w-full items-center gap-2 rounded-md px-1.5 py-1 text-left text-[11px] font-semibold tracking-[0.08em] transition-colors outline-none focus-visible:ring-[3px]"
                    onClick={() => toggleGroupCollapsed(group)}
                    aria-expanded={!isCollapsed}
                    aria-controls={groupListId}
                    aria-label={
                      isCollapsed
                        ? t("profiles.expandGroup", { group: groupLabel })
                        : t("profiles.collapseGroup", { group: groupLabel })
                    }
                  >
                    {isCollapsed ? (
                      <ChevronRight className="size-3 shrink-0" />
                    ) : (
                      <ChevronDown className="size-3 shrink-0" />
                    )}
                    <span className="min-w-0 flex-1 whitespace-pre-wrap">{groupLabel}</span>
                    <span className="shrink-0 tabular-nums">{items.length}</span>
                  </button>

                  {!isCollapsed && (
                    <div
                      id={groupListId}
                      className="space-y-2"
                      role="listbox"
                      aria-label={groupLabel}
                    >
                      {items.map((profile) => (
                        <ProfileRow
                          key={profile.id}
                          profile={profile}
                          isActive={profile.id === activeProfileId}
                          isDeleting={deletingId === profile.id}
                          onConnect={handleConnect}
                          onEdit={onEdit}
                          onDuplicate={onDuplicate}
                          onDelete={handleDelete}
                          onFocusRow={setSelectedProfileId}
                          rowRef={(node) => {
                            rowRefs.current[profile.id] = node
                          }}
                          t={t}
                        />
                      ))}
                    </div>
                  )}
                </section>
              )
            })}
          </div>
        )}
      </ScrollArea>
      <SshConfigImportDialog
        open={importDialogOpen}
        onOpenChange={setImportDialogOpen}
        onImported={(nextProfiles) => {
          setProfiles(nextProfiles)
        }}
      />
      <ConfirmDialog />
    </section>
  )
}
