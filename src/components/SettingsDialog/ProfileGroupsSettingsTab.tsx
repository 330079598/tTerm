import React from "react"
import { DragDropProvider } from "@dnd-kit/react"
import { invoke } from "@tauri-apps/api/core"
import { FolderTree, Plus, Trash2 } from "lucide-react"
import { useTranslation } from "react-i18next"

import {
  UNGROUPED_KEY,
  getGroupKey,
  getGroupNameFromKey,
  getProfileIdFromDndId,
  getGroupKeyFromDndId,
  getProfileDropIdFromDndId,
  moveProfileInList,
  type GroupColumn,
} from "@/components/SettingsDialog/profileGroupDnd"
import { ProfileGroupCard } from "@/components/SettingsDialog/ProfileGroupCard"
import { ProfileDragRow } from "@/components/SettingsDialog/ProfileDragRow"
import { SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import { useConfirmDialog } from "@/components/ui/app-dialog"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { ScrollArea } from "@/components/ui/scroll-area"
import { useConfig } from "@/contexts/ConfigContext"
import { useToast } from "@/hooks/use-toast"
import { invokeSafe } from "@/lib/errors"
import { isImeKeyEvent } from "@/lib/ime"
import { toErrorMessage } from "@/lib/utils"
import type { SavedProfile, Tab } from "@/types/tab"
import { findTunnelsUsingProfile } from "@/components/TunnelsPanel/profileTunnels"
import { summarizeTunnelNames } from "@/components/TunnelsPanel/tunnelUtils"
import { useProfileGroupActions } from "@/components/SettingsDialog/useProfileGroupActions"

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
  const { appState, saveAppState } = useConfig()
  const { confirm, ConfirmDialog } = useConfirmDialog()
  const [profileGroups, setProfileGroups] = React.useState<string[]>([])
  const [profiles, setProfiles] = React.useState<SavedProfile[]>([])
  const [newGroupName, setNewGroupName] = React.useState("")
  const [editingGroupName, setEditingGroupName] = React.useState<string | null>(null)
  const [editingGroupDraft, setEditingGroupDraft] = React.useState("")
  const [groupBusy, setGroupBusy] = React.useState(false)
  const [selectedProfileIds, setSelectedProfileIds] = React.useState<Set<string>>(new Set())
  const [profileSelectionBusy, setProfileSelectionBusy] = React.useState(false)
  const collapsedGroupKeySet = React.useMemo(
    () => new Set(appState.collapsedProfileGroupKeys),
    [appState.collapsedProfileGroupKeys]
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
    // eslint-disable-next-line react-hooks/set-state-in-effect -- state is only set after await; false positive fixed upstream in facebook/react#36734
    void refreshProfileGroups()
  }, [refreshKey, refreshProfileGroups])

  const [selectionProfiles, setSelectionProfiles] = React.useState(profiles)
  if (selectionProfiles !== profiles) {
    setSelectionProfiles(profiles)
    const knownIds = new Set(profiles.map((profile) => profile.id))
    setSelectedProfileIds((current) => {
      const next = new Set([...current].filter((id) => knownIds.has(id)))
      return next.size === current.size ? current : next
    })
  }

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

  const { handleAddGroup, startEditGroup, cancelEditGroup, handleRenameGroup, handleDeleteGroup } =
    useProfileGroupActions({
      newGroupName,
      setNewGroupName,
      profileGroups,
      setProfileGroups,
      profiles,
      setProfiles,
      editingGroupDraft,
      setEditingGroupName,
      setEditingGroupDraft,
      setGroupBusy,
      showGroupError,
      confirm,
      toast,
      t,
    })

  const toggleProfileSelected = React.useCallback((profileId: string) => {
    setSelectedProfileIds((current) => {
      const next = new Set(current)
      if (next.has(profileId)) {
        next.delete(profileId)
      } else {
        next.add(profileId)
      }
      return next
    })
  }, [])

  const clearProfileSelection = React.useCallback(() => {
    setSelectedProfileIds(new Set())
  }, [])

  const handleDeleteSelectedProfiles = React.useCallback(async () => {
    const ids = Array.from(selectedProfileIds)
    if (ids.length === 0) return

    const tunnelLists = await Promise.all(ids.map((id) => findTunnelsUsingProfile(id)))
    const tunnels = tunnelLists.flat()
    const tunnelNote =
      tunnels.length > 0
        ? `\n\n${t("tunnels.profileInUse", {
            count: tunnels.length,
            names: summarizeTunnelNames(tunnels),
            defaultValue:
              "{{count}} port-forwarding tunnel(s) use this host and will be stopped: {{names}}. Their rules are kept; choose another host for them afterwards.",
          })}`
        : ""

    const confirmed = await confirm({
      title: t("profiles.deleteSelected", { defaultValue: "Delete selected connections" }),
      description: `${t("profiles.deleteSelectedConfirm", {
        count: ids.length,
        defaultValue: "Delete these {{count}} connections?",
      })}${tunnelNote}`,
      confirmText: t("profiles.deleteSelected", { defaultValue: "Delete selected" }),
      cancelText: t("common.cancel"),
      variant: "destructive",
    })

    if (!confirmed) return

    setProfileSelectionBusy(true)
    try {
      await invoke("delete_profiles", { ids })
      const deleted = new Set(ids)
      setProfiles((current) => current.filter((item) => !deleted.has(item.id)))
      setSelectedProfileIds(new Set())
    } catch (error) {
      toast({
        title: t("profiles.deleteSelectedFailed", {
          defaultValue: "Failed to delete selected connections",
        }),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    } finally {
      setProfileSelectionBusy(false)
    }
  }, [confirm, selectedProfileIds, t, toast])

  const handleDeleteProfile = React.useCallback(
    async (profile: SavedProfile) => {
      const tunnels = await findTunnelsUsingProfile(profile.id)
      const tunnelNote =
        tunnels.length > 0
          ? `\n\n${t("tunnels.profileInUse", {
              count: tunnels.length,
              names: summarizeTunnelNames(tunnels),
              defaultValue:
                "{{count}} port-forwarding tunnel(s) use this host and will be stopped: {{names}}. Their rules are kept; choose another host for them afterwards.",
            })}`
          : ""
      const confirmed = await confirm({
        title: t("profiles.delete"),
        description: `${t("profiles.deleteConfirm")}\n\n${profile.name}${tunnelNote}`,
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

      saveAppState({ collapsedProfileGroupKeys: Array.from(next) }).catch((error) => {
        toast({
          title: t("settings.saveFailed", { defaultValue: "Failed to save settings" }),
          description: toErrorMessage(error),
          variant: "destructive",
        })
      })
    },
    [collapsedGroupKeySet, saveAppState, t, toast]
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
                    if (event.key === "Enter" && !isImeKeyEvent(event.nativeEvent)) {
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

            {selectedProfileIds.size > 0 && (
              <div className="bg-muted/40 flex items-center justify-between rounded-md border px-4 py-2">
                <span className="text-sm font-medium">
                  {t("profiles.selectedCount", {
                    count: selectedProfileIds.size,
                    defaultValue: "{{count}} connections selected",
                  })}
                </span>
                <div className="flex items-center gap-2">
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    onClick={clearProfileSelection}
                    disabled={profileSelectionBusy}
                  >
                    {t("profiles.clearSelection", { defaultValue: "Clear selection" })}
                  </Button>
                  <Button
                    type="button"
                    variant="destructive"
                    size="sm"
                    onClick={handleDeleteSelectedProfiles}
                    disabled={profileSelectionBusy}
                  >
                    <Trash2 size={14} />
                    {t("profiles.deleteSelected", { defaultValue: "Delete selected" })}
                  </Button>
                </div>
              </div>
            )}

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
                            isSelected={selectedProfileIds.has(profile.id)}
                            onConnectProfile={onConnectProfile}
                            onDeleteProfile={handleDeleteProfile}
                            onEditProfile={onEditProfile}
                            onToggleSelected={toggleProfileSelected}
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
