import React from "react"
import { DragDropProvider, useDraggable, useDroppable } from "@dnd-kit/react"
import { invoke } from "@tauri-apps/api/core"
import {
  Check,
  ChevronDown,
  ChevronRight,
  FolderTree,
  GripVertical,
  Pencil,
  Plus,
  PlugZap,
  Server,
  Terminal,
  Trash2,
  X,
} from "lucide-react"
import { useTranslation } from "react-i18next"

import { SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import { useConfirmDialog } from "@/components/ui/app-dialog"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { useConfig } from "@/contexts/ConfigContext"
import { useToast } from "@/hooks/use-toast"
import { invokeSafe } from "@/lib/errors"
import { buildConnectionFromProfile } from "@/lib/profileConnections"
import { cn, toErrorMessage } from "@/lib/utils"
import type { SavedProfile, Tab } from "@/types/tab"

const UNGROUPED_KEY = "__ungrouped__"

const getGroupKey = (group: string) => (group.trim() ? group.trim() : UNGROUPED_KEY)
const getGroupNameFromKey = (key: string) => (key === UNGROUPED_KEY ? "" : key)
const profileDragId = (profileId: string) => `profile:${profileId}:drag`
const profileDropId = (profileId: string) => `profile:${profileId}:drop`
const groupDropId = (groupKey: string) => `profile-group:${encodeURIComponent(groupKey)}:drop`
const groupListId = (groupKey: string) => `profile-group-${encodeURIComponent(groupKey)}-list`

const getProfileIdFromDndId = (id?: string | number | null): string | null => {
  if (!id) return null
  const parts = String(id).split(":")
  if (parts.length < 3 || parts[0] !== "profile") return null
  return parts[1] || null
}

const getGroupKeyFromDndId = (id?: string | number | null): string | null => {
  if (!id) return null
  const parts = String(id).split(":")
  if (parts.length < 3 || parts[0] !== "profile-group") return null
  return parts[1] ? decodeURIComponent(parts[1]) : null
}

const getProfileDropIdFromDndId = (id?: string | number | null): string | null => {
  if (!id) return null
  const parts = String(id).split(":")
  if (parts.length < 3 || parts[0] !== "profile" || parts[2] !== "drop") return null
  return parts[1] || null
}

const buildProfileSubtitle = (
  profile: SavedProfile,
  t: (key: string, options?: Record<string, unknown>) => string
) => {
  if (profile.connection_type === "terminal") {
    return t("profiles.localTerminal")
  }

  if (!profile.host) {
    return t("profiles.connectionDetailsUnavailable")
  }

  return `${profile.username ? `${profile.username}@` : ""}${profile.host}${
    profile.port && profile.port !== 22 ? `:${profile.port}` : ""
  }`
}

const moveProfileInList = (
  profiles: SavedProfile[],
  profileId: string,
  targetGroup: string,
  targetProfileId: string | null
) => {
  const sourceIndex = profiles.findIndex((profile) => profile.id === profileId)
  if (sourceIndex < 0) {
    return profiles
  }

  const nextProfiles = [...profiles]
  const [profile] = nextProfiles.splice(sourceIndex, 1)
  const movedProfile = { ...profile, group: targetGroup }
  const targetIndex = targetProfileId
    ? nextProfiles.findIndex((profile) => profile.id === targetProfileId)
    : -1

  nextProfiles.splice(targetIndex >= 0 ? targetIndex : nextProfiles.length, 0, movedProfile)
  return nextProfiles
}

interface GroupColumn {
  key: string
  name: string
  profiles: SavedProfile[]
}

interface ProfileGroupCardProps {
  children: React.ReactNode
  count: number
  group: GroupColumn
  groupBusy: boolean
  isCollapsed: boolean
  isEditing: boolean
  editingGroupDraft: string
  onCancelEdit: () => void
  onDeleteGroup: (name: string) => void
  onRenameGroup: (oldName: string) => void
  onStartEdit: (name: string) => void
  onToggleCollapsed: (groupKey: string) => void
  setEditingGroupDraft: React.Dispatch<React.SetStateAction<string>>
}

const ProfileGroupCard: React.FC<ProfileGroupCardProps> = ({
  children,
  count,
  group,
  groupBusy,
  isCollapsed,
  isEditing,
  editingGroupDraft,
  onCancelEdit,
  onDeleteGroup,
  onRenameGroup,
  onStartEdit,
  onToggleCollapsed,
  setEditingGroupDraft,
}) => {
  const { t } = useTranslation()
  const { ref, isDropTarget } = useDroppable({ id: groupDropId(group.key) })
  const isUngrouped = group.key === UNGROUPED_KEY
  const displayName = isUngrouped ? t("profiles.ungrouped") : group.name
  const listId = groupListId(group.key)
  const toggleLabel = isCollapsed
    ? t("profiles.expandGroup", { group: displayName })
    : t("profiles.collapseGroup", { group: displayName })

  return (
    <section
      ref={ref}
      className={cn(
        "border-border/70 bg-card flex flex-col rounded-md border transition-colors",
        isCollapsed ? "min-h-0" : "min-h-[220px]",
        isDropTarget && "border-primary/70 bg-primary/5"
      )}
    >
      <div className="border-border/70 flex items-start gap-3 border-b px-4 py-3">
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              className="-ml-1 shrink-0"
              onClick={() => onToggleCollapsed(group.key)}
              aria-expanded={!isCollapsed}
              aria-controls={listId}
              aria-label={toggleLabel}
            >
              {isCollapsed ? <ChevronRight size={14} /> : <ChevronDown size={14} />}
            </Button>
          </TooltipTrigger>
          <TooltipContent>{toggleLabel}</TooltipContent>
        </Tooltip>
        <div className="bg-muted text-muted-foreground flex size-8 shrink-0 items-center justify-center rounded-md border">
          <FolderTree size={15} />
        </div>
        <div className="min-w-0 flex-1">
          {isEditing ? (
            <Input
              value={editingGroupDraft}
              onChange={(event) => setEditingGroupDraft(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault()
                  onRenameGroup(group.name)
                }
                if (event.key === "Escape") {
                  onCancelEdit()
                }
              }}
              disabled={groupBusy}
              className="h-8"
              autoFocus
            />
          ) : (
            <>
              <div className="truncate text-sm font-semibold">{displayName}</div>
              <div className="text-muted-foreground mt-1 text-xs">
                {t("profileGroups.profileCount", {
                  count,
                  defaultValue: "{{count}} saved connections",
                })}
              </div>
            </>
          )}
        </div>

        {!isUngrouped && (
          <div className="flex shrink-0 items-center gap-1">
            {isEditing ? (
              <>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-sm"
                      onClick={() => onRenameGroup(group.name)}
                      disabled={groupBusy || !editingGroupDraft.trim()}
                      aria-label={t("common.save")}
                    >
                      <Check size={14} />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>{t("common.save")}</TooltipContent>
                </Tooltip>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-sm"
                      onClick={onCancelEdit}
                      disabled={groupBusy}
                      aria-label={t("common.cancel")}
                    >
                      <X size={14} />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>{t("common.cancel")}</TooltipContent>
                </Tooltip>
              </>
            ) : (
              <>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-sm"
                      onClick={() => onStartEdit(group.name)}
                      disabled={groupBusy}
                      aria-label={t("profileGroups.rename")}
                    >
                      <Pencil size={14} />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>{t("profileGroups.rename")}</TooltipContent>
                </Tooltip>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-sm"
                      className="hover:text-destructive"
                      onClick={() => onDeleteGroup(group.name)}
                      disabled={groupBusy}
                      aria-label={t("profileGroups.delete")}
                    >
                      <Trash2 size={14} />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>{t("profileGroups.delete")}</TooltipContent>
                </Tooltip>
              </>
            )}
          </div>
        )}
      </div>

      {!isCollapsed && (
        <div id={listId} className="flex flex-1 flex-col gap-2 p-3">
          {children}
        </div>
      )}
    </section>
  )
}

interface ProfileDragRowProps {
  profile: SavedProfile
  onConnectProfile?: (connection: Omit<Tab, "id" | "isActive">) => void
  onDeleteProfile: (profile: SavedProfile) => void
  onEditProfile?: (profile: SavedProfile) => void
}

const ProfileDragRow: React.FC<ProfileDragRowProps> = ({
  profile,
  onConnectProfile,
  onDeleteProfile,
  onEditProfile,
}) => {
  const { t } = useTranslation()
  const { ref: draggableRef, isDragging } = useDraggable({ id: profileDragId(profile.id) })
  const { ref: droppableRef, isDropTarget } = useDroppable({ id: profileDropId(profile.id) })
  const isTerminal = profile.connection_type === "terminal"
  const Icon = isTerminal ? Terminal : Server
  const hasProfileActions = Boolean(onConnectProfile || onEditProfile || onDeleteProfile)
  const setNodeRef = (node: HTMLDivElement | null) => {
    draggableRef(node)
    droppableRef(node)
  }

  return (
    <div
      ref={setNodeRef}
      className={cn(
        "group border-border/70 bg-background/70 hover:bg-muted/35 focus-visible:ring-ring/50 flex cursor-grab items-start gap-3 rounded-md border px-3 py-2.5 text-left transition-colors outline-none focus-visible:ring-[3px]",
        isDragging && "cursor-grabbing opacity-60",
        isDropTarget && "border-primary/70 bg-primary/5"
      )}
      role="listitem"
      tabIndex={0}
    >
      <GripVertical size={15} className="text-muted-foreground mt-0.5 shrink-0" />
      <Icon size={15} className="text-muted-foreground mt-0.5 shrink-0" />
      <div className="min-w-0 flex-1">
        <div className="flex min-w-0 items-center gap-2">
          <div className="truncate text-sm font-medium">{profile.name}</div>
          <Badge variant="outline" className="h-5 shrink-0 px-1.5 text-[10px]">
            {isTerminal ? t("profiles.localBadge") : "SSH"}
          </Badge>
        </div>
        <div className="text-muted-foreground mt-1 truncate text-xs">
          {buildProfileSubtitle(profile, t)}
        </div>
      </div>
      {hasProfileActions && (
        <div className="flex shrink-0 items-center gap-1 self-center opacity-0 transition-opacity group-focus-within:opacity-100 group-hover:opacity-100">
          {onConnectProfile && (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  aria-label={t("profiles.connect")}
                  onClick={(event) => {
                    event.stopPropagation()
                    onConnectProfile(buildConnectionFromProfile(profile))
                  }}
                >
                  <PlugZap size={13} />
                </Button>
              </TooltipTrigger>
              <TooltipContent>{t("profiles.connect")}</TooltipContent>
            </Tooltip>
          )}
          {onEditProfile && (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  aria-label={t("profiles.edit")}
                  onClick={(event) => {
                    event.stopPropagation()
                    onEditProfile(profile)
                  }}
                >
                  <Pencil size={13} />
                </Button>
              </TooltipTrigger>
              <TooltipContent>{t("profiles.edit")}</TooltipContent>
            </Tooltip>
          )}
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                className="hover:text-destructive"
                aria-label={t("profiles.delete")}
                onClick={(event) => {
                  event.stopPropagation()
                  onDeleteProfile(profile)
                }}
              >
                <Trash2 size={13} />
              </Button>
            </TooltipTrigger>
            <TooltipContent>{t("profiles.delete")}</TooltipContent>
          </Tooltip>
        </div>
      )}
    </div>
  )
}

interface ProfileGroupsSettingsTabProps {
  onConnectProfile?: (connection: Omit<Tab, "id" | "isActive">) => void
  onEditProfile?: (profile: SavedProfile) => void
  refreshKey?: number
}

export const ProfileGroupsSettingsTab: React.FC<ProfileGroupsSettingsTabProps> = ({
  onConnectProfile,
  onEditProfile,
  refreshKey,
}) => {
  const { t } = useTranslation()
  const { toast } = useToast()
  const { config, saveConfig } = useConfig()
  const { confirm, ConfirmDialog } = useConfirmDialog()
  const [profileGroups, setProfileGroups] = React.useState<string[]>([])
  const [profiles, setProfiles] = React.useState<SavedProfile[]>([])
  const [newGroupName, setNewGroupName] = React.useState("")
  const [editingGroupName, setEditingGroupName] = React.useState<string | null>(null)
  const [editingGroupDraft, setEditingGroupDraft] = React.useState("")
  const [groupBusy, setGroupBusy] = React.useState(false)
  const collapsedGroupKeySet = React.useMemo(
    () => new Set(config.collapsed_profile_group_keys),
    [config.collapsed_profile_group_keys]
  )

  const refreshProfileGroups = React.useCallback(async () => {
    const groupsResult = await invokeSafe<string[]>("list_profile_groups", undefined, {
      context: "list_profile_groups",
      title: t("errors.profileGroupsLoadFailed"),
    })
    if (!groupsResult.ok) return

    const profilesResult = await invokeSafe<SavedProfile[]>("list_profiles", undefined, {
      context: "list_profiles",
      title: t("errors.profileGroupsLoadFailed"),
    })
    if (!profilesResult.ok) return

    setProfileGroups(groupsResult.value)
    setProfiles(profilesResult.value)
  }, [t])

  React.useEffect(() => {
    void refreshProfileGroups()
  }, [refreshKey, refreshProfileGroups])

  const showGroupError = React.useCallback(
    (error: unknown) => {
      toast({
        title: t("profileGroups.saveFailed", { defaultValue: "Failed to save group" }),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    },
    [t, toast]
  )

  const groupColumns = React.useMemo<GroupColumn[]>(() => {
    const knownGroups = new Set(profileGroups)
    for (const profile of profiles) {
      const group = profile.group?.trim()
      if (group) {
        knownGroups.add(group)
      }
    }

    const groups = Array.from(knownGroups).sort((left, right) => left.localeCompare(right))
    const columns: GroupColumn[] = groups.map((group) => ({
      key: getGroupKey(group),
      name: group,
      profiles: [],
    }))
    columns.push({ key: UNGROUPED_KEY, name: "", profiles: [] })

    const byKey = new Map(columns.map((column) => [column.key, column]))
    for (const profile of profiles) {
      const key = getGroupKey(profile.group ?? "")
      const column = byKey.get(key) ?? byKey.get(UNGROUPED_KEY)
      column?.profiles.push(profile)
    }

    return columns
  }, [profileGroups, profiles])

  const handleAddGroup = React.useCallback(async () => {
    const name = newGroupName.trim()
    if (!name) return

    if (profileGroups.includes(name)) {
      toast({
        title: t("profileGroups.duplicate", { defaultValue: "Group already exists" }),
        variant: "destructive",
      })
      return
    }

    setGroupBusy(true)
    try {
      const groups = await invoke<string[]>("save_profile_group", { name })
      setProfileGroups(groups)
      setNewGroupName("")
    } catch (error) {
      showGroupError(error)
    } finally {
      setGroupBusy(false)
    }
  }, [newGroupName, profileGroups, showGroupError, t, toast])

  const startEditGroup = React.useCallback((group: string) => {
    setEditingGroupName(group)
    setEditingGroupDraft(group)
  }, [])

  const cancelEditGroup = React.useCallback(() => {
    setEditingGroupName(null)
    setEditingGroupDraft("")
  }, [])

  const handleRenameGroup = React.useCallback(
    async (oldName: string) => {
      const newName = editingGroupDraft.trim()
      if (!newName || newName === oldName) {
        cancelEditGroup()
        return
      }

      if (profileGroups.some((group) => group !== oldName && group === newName)) {
        toast({
          title: t("profileGroups.duplicate", { defaultValue: "Group already exists" }),
          variant: "destructive",
        })
        return
      }

      setGroupBusy(true)
      try {
        const groups = await invoke<string[]>("rename_profile_group", {
          oldName,
          newName,
        })
        setProfileGroups(groups)
        setProfiles((current) =>
          current.map((profile) =>
            profile.group?.trim() === oldName ? { ...profile, group: newName } : profile
          )
        )
        cancelEditGroup()
      } catch (error) {
        showGroupError(error)
      } finally {
        setGroupBusy(false)
      }
    },
    [cancelEditGroup, editingGroupDraft, profileGroups, showGroupError, t, toast]
  )

  const handleDeleteGroup = React.useCallback(
    async (name: string) => {
      const count = profiles.filter((profile) => profile.group?.trim() === name).length
      const confirmed = await confirm({
        title: t("profileGroups.delete", { defaultValue: "Delete group" }),
        description: t("profileGroups.deleteConfirm", {
          count,
          defaultValue:
            "Delete this group? Saved connections in this group will move to Ungrouped.",
        }),
        confirmText: t("profileGroups.delete", { defaultValue: "Delete group" }),
        cancelText: t("common.cancel"),
        variant: "destructive",
      })

      if (!confirmed) return

      setGroupBusy(true)
      try {
        const groups = await invoke<string[]>("delete_profile_group", { name })
        setProfileGroups(groups)
        setProfiles((current) =>
          current.map((profile) =>
            profile.group?.trim() === name ? { ...profile, group: "" } : profile
          )
        )
      } catch (error) {
        showGroupError(error)
      } finally {
        setGroupBusy(false)
      }
    },
    [confirm, profiles, showGroupError, t]
  )

  const handleDeleteProfile = React.useCallback(
    async (profile: SavedProfile) => {
      const confirmed = await confirm({
        title: t("profiles.delete"),
        description: `${t("profiles.deleteConfirm")}\n\n${profile.name}`,
        confirmText: t("profiles.delete"),
        cancelText: t("common.cancel"),
        variant: "destructive",
      })

      if (!confirmed) return

      try {
        await invoke("delete_profile", { id: profile.id })
        setProfiles((current) => current.filter((item) => item.id !== profile.id))
      } catch (error) {
        toast({
          title: t("profiles.delete"),
          description: toErrorMessage(error),
          variant: "destructive",
        })
      }
    },
    [confirm, t, toast]
  )

  const getProfileGroupKey = React.useCallback(
    (profileId: string) => getGroupKey(profiles.find((item) => item.id === profileId)?.group ?? ""),
    [profiles]
  )

  const handleMoveProfile = React.useCallback(
    async (profileId: string, targetGroupKey: string, targetProfileId?: string | null) => {
      if (profileId === targetProfileId) {
        return
      }

      const targetGroup =
        targetProfileId && getProfileGroupKey(targetProfileId)
          ? getGroupNameFromKey(getProfileGroupKey(targetProfileId))
          : getGroupNameFromKey(targetGroupKey)
      const profile = profiles.find((item) => item.id === profileId)
      if (!profile) {
        return
      }

      const currentIndex = profiles.findIndex((item) => item.id === profileId)
      const currentTargetIndex = targetProfileId
        ? profiles.findIndex((item) => item.id === targetProfileId)
        : -1
      const sameGroup = (profile.group ?? "").trim() === targetGroup
      if (sameGroup && targetProfileId && currentIndex === currentTargetIndex - 1) {
        return
      }
      if (sameGroup && !targetProfileId) {
        const groupProfiles = profiles.filter(
          (item) => getGroupKey(item.group ?? "") === targetGroupKey
        )
        if (groupProfiles[groupProfiles.length - 1]?.id === profileId) {
          return
        }
      }

      setProfiles((current) =>
        moveProfileInList(current, profileId, targetGroup, targetProfileId ?? null)
      )

      try {
        await invoke("move_profile_to_group", {
          id: profileId,
          group: targetGroup,
          targetId: targetProfileId ?? null,
        })
        if (targetGroup) {
          setProfileGroups((current) =>
            current.includes(targetGroup)
              ? current
              : [...current, targetGroup].sort((left, right) => left.localeCompare(right))
          )
        }
      } catch (error) {
        await refreshProfileGroups()
        toast({
          title: t("profileGroups.moveFailed", { defaultValue: "Failed to move connection" }),
          description: toErrorMessage(error),
          variant: "destructive",
        })
      }
    },
    [getProfileGroupKey, profiles, refreshProfileGroups, t, toast]
  )

  const toggleGroupCollapsed = React.useCallback(
    (groupKey: string) => {
      const next = new Set(collapsedGroupKeySet)
      if (next.has(groupKey)) {
        next.delete(groupKey)
      } else {
        next.add(groupKey)
      }

      saveConfig({ collapsed_profile_group_keys: Array.from(next) }).catch((error) => {
        toast({
          title: t("settings.saveFailed", { defaultValue: "Failed to save settings" }),
          description: toErrorMessage(error),
          variant: "destructive",
        })
      })
    },
    [collapsedGroupKeySet, saveConfig, t, toast]
  )

  const handleDragEnd = React.useCallback(
    (
      event: Parameters<NonNullable<React.ComponentProps<typeof DragDropProvider>["onDragEnd"]>>[0]
    ) => {
      if (event.canceled) return

      const profileId = getProfileIdFromDndId(event.operation.source?.id ?? null)
      const targetId = event.operation.target?.id ?? null
      const targetProfileId = getProfileDropIdFromDndId(targetId)
      const targetGroupKey =
        (targetProfileId ? getProfileGroupKey(targetProfileId) : null) ??
        getGroupKeyFromDndId(targetId)
      if (!profileId || !targetGroupKey) {
        return
      }

      void handleMoveProfile(profileId, targetGroupKey, targetProfileId)
    },
    [getProfileGroupKey, handleMoveProfile]
  )

  return (
    <>
      <ScrollArea className="h-full pr-4">
        <div className="space-y-6">
          <SettingsSection
            icon={<FolderTree size={16} />}
            title={t("profileGroups.title")}
            description={t("profileGroups.pageDescription", {
              defaultValue:
                "Create groups and drag saved connection profiles between them to keep your workspace organized.",
            })}
          >
            <div className="bg-card rounded-md border p-4">
              <div className="flex max-w-lg gap-2">
                <Input
                  value={newGroupName}
                  onChange={(event) => setNewGroupName(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") {
                      event.preventDefault()
                      void handleAddGroup()
                    }
                  }}
                  placeholder={t("profileGroups.namePlaceholder")}
                  disabled={groupBusy}
                  className="h-8"
                />
                <Button
                  type="button"
                  size="sm"
                  onClick={handleAddGroup}
                  disabled={groupBusy || !newGroupName.trim()}
                >
                  <Plus size={14} />
                  {t("profileGroups.addButton")}
                </Button>
              </div>
            </div>

            <DragDropProvider onDragEnd={handleDragEnd}>
              <div className="grid grid-cols-1 gap-3 xl:grid-cols-2">
                {groupColumns.map((group) => (
                  <ProfileGroupCard
                    key={group.key}
                    group={group}
                    count={group.profiles.length}
                    groupBusy={groupBusy}
                    isCollapsed={collapsedGroupKeySet.has(group.key)}
                    isEditing={editingGroupName === group.name}
                    editingGroupDraft={editingGroupDraft}
                    onCancelEdit={cancelEditGroup}
                    onDeleteGroup={handleDeleteGroup}
                    onRenameGroup={handleRenameGroup}
                    onStartEdit={startEditGroup}
                    onToggleCollapsed={toggleGroupCollapsed}
                    setEditingGroupDraft={setEditingGroupDraft}
                  >
                    {group.profiles.length === 0 ? (
                      <div className="text-muted-foreground flex min-h-20 items-center justify-center rounded-md border border-dashed px-4 text-center text-xs">
                        {t("profileGroups.dropEmpty", {
                          defaultValue: "Drop saved connections here.",
                        })}
                      </div>
                    ) : (
                      <div className="space-y-2" role="list">
                        {group.profiles.map((profile) => (
                          <ProfileDragRow
                            key={profile.id}
                            profile={profile}
                            onConnectProfile={onConnectProfile}
                            onDeleteProfile={handleDeleteProfile}
                            onEditProfile={onEditProfile}
                          />
                        ))}
                      </div>
                    )}
                  </ProfileGroupCard>
                ))}
              </div>
            </DragDropProvider>
          </SettingsSection>
        </div>
      </ScrollArea>
      <ConfirmDialog />
    </>
  )
}
