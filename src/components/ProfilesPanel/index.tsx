import React, { useEffect, useMemo, useRef, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { useTranslation } from "react-i18next"
import { ChevronDown, ChevronRight, FileInput, Plus, Search, X } from "lucide-react"

import {
  filterProfiles,
  groupProfiles,
  UNGROUPED_KEY,
} from "@/components/ProfilesPanel/profileListing"
import { ProfileRow } from "@/components/ProfilesPanel/ProfileRow"
import { ProfileSortToggle } from "@/components/ProfilesPanel/ProfileSortToggle"
import { useProfileSort } from "@/components/ProfilesPanel/useProfileSort"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { Button } from "@/components/ui/button"
import { useToast } from "@/hooks/use-toast"
import { Input } from "@/components/ui/input"
import { ScrollArea } from "@/components/ui/scroll-area"
import { useConfirmDialog } from "@/components/ui/app-dialog"
import { SshConfigImportDialog } from "@/components/SshConfigImportDialog"
import { isImeKeyEvent } from "@/lib/ime"
import { buildConnectionFromProfile } from "@/lib/profileConnections"
import { profileLastUsed, type RecentProfile } from "@/lib/profileRecents"
import { rankProfiles } from "@/lib/profileSearch"
import { cn } from "@/lib/utils"
import { Tab, type SavedProfile } from "@/types/tab"
import { findTunnelsUsingProfile } from "@/components/TunnelsPanel/profileTunnels"
import { summarizeTunnelNames } from "@/components/TunnelsPanel/tunnelUtils"

interface ProfilesPanelProps {
  onConnect: (connection: Omit<Tab, "id" | "isActive">) => void
  onEdit: (profile: SavedProfile) => void
  onDuplicate: (profile: SavedProfile) => void
  onDeleteProfile?: (profile: SavedProfile) => void
  collapsedGroupKeys?: string[]
  /** The saved connections used lately, for listing them by last use. */
  profileRecents?: readonly RecentProfile[]
  refreshKey?: number
  onCreate?: () => void
  onClose?: () => void
  onCollapsedGroupKeysChange?: (groups: string[]) => void
  surface?: "panel" | "plain"
  className?: string
}

export type { SavedProfile }

const getGroupListId = (group: string) => `profiles-group-${encodeURIComponent(group)}`

export const ProfilesPanel: React.FC<ProfilesPanelProps> = ({
  onConnect,
  onEdit,
  onDuplicate,
  onDeleteProfile,
  collapsedGroupKeys = [],
  profileRecents = [],
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
  const [sort, setSort] = useProfileSort()
  const { confirm, ConfirmDialog } = useConfirmDialog()
  const collapsedGroupKeySet = useMemo(() => new Set(collapsedGroupKeys), [collapsedGroupKeys])

  const [loadingRefreshKey, setLoadingRefreshKey] = useState(refreshKey)
  if (loadingRefreshKey !== refreshKey) {
    setLoadingRefreshKey(refreshKey)
    setIsLoading(true)
  }

  useEffect(() => {
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
    const tunnels = await findTunnelsUsingProfile(id)
    let deletePrompt = profile
      ? `${t("profiles.deleteConfirm")}\n\n${profile.name}`
      : t("profiles.deleteConfirm")
    if (tunnels.length > 0) {
      deletePrompt += `\n\n${t("tunnels.profileInUse", {
        count: tunnels.length,
        names: summarizeTunnelNames(tunnels),
        defaultValue:
          "{{count}} port-forwarding tunnel(s) use this host and will be stopped: {{names}}. Their rules are kept; choose another host for them afterwards.",
      })}`
    }

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
      if (profile) {
        onDeleteProfile?.(profile)
      }
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

  const filteredProfiles = useMemo(
    () => filterProfiles(profiles, searchQuery),
    [profiles, searchQuery]
  )
  // Listed by last use, the profiles form one list without groups, and a
  // search still puts its best fits first.
  const recentProfiles = useMemo(
    () =>
      sort === "recent"
        ? rankProfiles(filteredProfiles, searchQuery, profileLastUsed(profileRecents))
        : null,
    [filteredProfiles, profileRecents, searchQuery, sort]
  )
  const groupedProfiles = useMemo(
    () => groupProfiles(filteredProfiles, searchQuery),
    [filteredProfiles, searchQuery]
  )

  const flatProfiles = useMemo(
    () =>
      recentProfiles ??
      groupedProfiles.flatMap(([group, items]) =>
        collapsedGroupKeySet.has(group) && !searchQuery.trim() ? [] : items
      ),
    [collapsedGroupKeySet, groupedProfiles, recentProfiles, searchQuery]
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

  const handleSearchKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (isImeKeyEvent(event.nativeEvent) || flatProfiles.length === 0) return
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault()
      const step = event.key === "ArrowDown" ? 1 : -1
      const index = flatProfiles.findIndex((profile) => profile.id === activeProfileId)
      const next = (flatProfiles.length + index + step) % flatProfiles.length
      setSelectedProfileId(flatProfiles[next].id)
    } else if (event.key === "Enter") {
      event.preventDefault()
      const profile = flatProfiles.find((item) => item.id === activeProfileId)
      if (profile) handleConnect(profile)
    }
  }

  const hasProfiles = profiles.length > 0
  const hasFilteredResults = filteredProfiles.length > 0
  const isFiltering = searchQuery.trim().length > 0

  const renderRow = (profile: SavedProfile, showGroup: boolean) => (
    <ProfileRow
      key={profile.id}
      profile={profile}
      showGroup={showGroup}
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
  )

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

      <div className="flex gap-2 px-5 pt-4 pb-3">
        <div className="relative min-w-0 flex-1">
          <Search className="text-muted-foreground pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2" />
          <Input
            ref={searchInputRef}
            type="search"
            value={searchQuery}
            onChange={(event) => {
              setSearchQuery(event.target.value)
              setSelectedProfileId(null)
            }}
            onKeyDown={handleSearchKeyDown}
            placeholder={t("profiles.searchPlaceholder")}
            className="pl-9"
          />
        </div>
        <ProfileSortToggle
          sort={sort}
          onSortChange={(next) => {
            setSort(next)
            setSelectedProfileId(null)
          }}
        />
      </div>

      <ScrollArea className="flex-1 px-3 pb-3">
        {isLoading && (
          <div className="flex min-h-[260px] flex-col items-center justify-center">
            <Spinner size={24} className="text-muted-foreground" />
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

        {hasFilteredResults && recentProfiles && (
          <div
            className="space-y-2 px-2 pb-2"
            role="listbox"
            aria-label={t("profiles.sort.recent")}
          >
            {recentProfiles.map((profile) => renderRow(profile, true))}
          </div>
        )}

        {hasFilteredResults && !recentProfiles && (
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
                      {items.map((profile) => renderRow(profile, false))}
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
