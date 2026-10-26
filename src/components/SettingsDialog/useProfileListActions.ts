import React from "react"
import type { DragDropProvider } from "@dnd-kit/react"
import { invoke } from "@tauri-apps/api/core"
import { useTranslation } from "react-i18next"

import {
  getGroupIdFromDragId,
  getGroupIdFromKey,
  getGroupKey,
  getGroupKeyFromDropId,
  getProfileDropIdFromDndId,
  getProfileIdFromDndId,
  moveProfileInList,
  ROOT_DROP_ID,
  UNGROUPED_KEY,
} from "@/components/SettingsDialog/profileGroupDnd"
import { profileTunnelNote } from "@/components/SettingsDialog/profileDeletion"
import { findTunnelsUsingProfile } from "@/components/TunnelsPanel/profileTunnels"
import type { useConfirmDialog } from "@/components/ui/app-dialog"
import { useToast } from "@/hooks/use-toast"
import { groupAndDescendantIds, type GroupTree } from "@/lib/profileGroups"
import { toErrorMessage } from "@/lib/utils"
import type { ProfileGroup, SavedProfile } from "@/types/tab"

interface ProfileListActionsOptions {
  profiles: SavedProfile[]
  setProfiles: React.Dispatch<React.SetStateAction<SavedProfile[]>>
  groupTree: GroupTree
  moveGroup: (group: ProfileGroup, parentId: string | null) => Promise<void>
  refreshProfileGroups: () => Promise<void>
  confirm: ReturnType<typeof useConfirmDialog>["confirm"]
}

/**
 * Deleting one host, dragging hosts between groups or within one, and
 * dragging a group into another or to the top.
 */
export function useProfileListActions({
  profiles,
  setProfiles,
  groupTree,
  moveGroup,
  refreshProfileGroups,
  confirm,
}: ProfileListActionsOptions) {
  const { t } = useTranslation()
  const { toast } = useToast()
  const [draggingGroupId, setDraggingGroupId] = React.useState<string | null>(null)

  const handleDeleteProfile = React.useCallback(
    async (profile: SavedProfile) => {
      const tunnels = await findTunnelsUsingProfile(profile.id)
      const tunnelNote = profileTunnelNote(t, tunnels)
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
    [confirm, setProfiles, t, toast]
  )

  const getProfileGroupKey = React.useCallback(
    (profileId: string) => getGroupKey(profiles.find((item) => item.id === profileId)?.group_id),
    [profiles]
  )

  const handleMoveProfile = React.useCallback(
    async (profileId: string, targetGroupKey: string, targetProfileId?: string | null) => {
      if (profileId === targetProfileId) {
        return
      }

      const targetGroupId = getGroupIdFromKey(
        targetProfileId ? getProfileGroupKey(targetProfileId) : targetGroupKey
      )
      const profile = profiles.find((item) => item.id === profileId)
      if (!profile) {
        return
      }

      const currentIndex = profiles.findIndex((item) => item.id === profileId)
      const currentTargetIndex = targetProfileId
        ? profiles.findIndex((item) => item.id === targetProfileId)
        : -1
      const sameGroup = getGroupKey(profile.group_id) === getGroupKey(targetGroupId)
      if (sameGroup && targetProfileId && currentIndex === currentTargetIndex - 1) {
        return
      }
      if (sameGroup && !targetProfileId) {
        const groupProfiles = profiles.filter(
          (item) => getGroupKey(item.group_id) === getGroupKey(targetGroupId)
        )
        if (groupProfiles[groupProfiles.length - 1]?.id === profileId) {
          return
        }
      }

      setProfiles((current) =>
        moveProfileInList(current, profileId, targetGroupId, targetProfileId ?? null)
      )

      try {
        await invoke("move_profile_to_group", {
          id: profileId,
          groupId: targetGroupId ?? null,
          targetId: targetProfileId ?? null,
        })
      } catch (error) {
        await refreshProfileGroups()
        toast({
          title: t("profileGroups.moveFailed", { defaultValue: "Failed to move connection" }),
          description: toErrorMessage(error),
          variant: "destructive",
        })
      }
    },
    [getProfileGroupKey, profiles, refreshProfileGroups, setProfiles, t, toast]
  )

  /** Files a dragged group under the group it was dropped on, or at the top. */
  const handleMoveGroup = React.useCallback(
    (groupId: string, targetId: string | number | null) => {
      const node = groupTree.nodes.get(groupId)
      if (!node) return
      const targetProfileId = getProfileDropIdFromDndId(targetId)
      const targetKey =
        targetId === ROOT_DROP_ID
          ? UNGROUPED_KEY
          : targetProfileId
            ? getProfileGroupKey(targetProfileId)
            : getGroupKeyFromDropId(targetId)
      if (!targetKey) return
      const parentId = getGroupIdFromKey(targetKey) ?? null
      // A group can't go into itself or a group inside it.
      if (parentId && groupAndDescendantIds(groupTree, groupId).has(parentId)) return
      void moveGroup(node.group, parentId)
    },
    [getProfileGroupKey, groupTree, moveGroup]
  )

  const handleDragStart = React.useCallback(
    (
      event: Parameters<
        NonNullable<React.ComponentProps<typeof DragDropProvider>["onDragStart"]>
      >[0]
    ) => {
      setDraggingGroupId(getGroupIdFromDragId(event.operation.source?.id ?? null))
    },
    []
  )

  const handleDragEnd = React.useCallback(
    (
      event: Parameters<NonNullable<React.ComponentProps<typeof DragDropProvider>["onDragEnd"]>>[0]
    ) => {
      setDraggingGroupId(null)
      if (event.canceled) return

      const sourceId = event.operation.source?.id ?? null
      const targetId = event.operation.target?.id ?? null
      const groupId = getGroupIdFromDragId(sourceId)
      if (groupId) {
        handleMoveGroup(groupId, targetId)
        return
      }

      const profileId = getProfileIdFromDndId(sourceId)
      const targetProfileId = getProfileDropIdFromDndId(targetId)
      const targetGroupKey =
        (targetProfileId ? getProfileGroupKey(targetProfileId) : null) ??
        getGroupKeyFromDropId(targetId)
      if (!profileId || !targetGroupKey) {
        return
      }

      void handleMoveProfile(profileId, targetGroupKey, targetProfileId)
    },
    [getProfileGroupKey, handleMoveGroup, handleMoveProfile]
  )

  return { handleDeleteProfile, handleDragStart, handleDragEnd, draggingGroupId }
}
