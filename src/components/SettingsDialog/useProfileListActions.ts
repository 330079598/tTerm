import React from "react"
import type { DragDropProvider } from "@dnd-kit/react"
import { invoke } from "@tauri-apps/api/core"
import { useTranslation } from "react-i18next"

import {
  getGroupKey,
  getGroupKeyFromDndId,
  getGroupNameFromKey,
  getProfileDropIdFromDndId,
  getProfileIdFromDndId,
  moveProfileInList,
} from "@/components/SettingsDialog/profileGroupDnd"
import { profileTunnelNote } from "@/components/SettingsDialog/profileDeletion"
import { findTunnelsUsingProfile } from "@/components/TunnelsPanel/profileTunnels"
import type { useConfirmDialog } from "@/components/ui/app-dialog"
import { useToast } from "@/hooks/use-toast"
import { toErrorMessage } from "@/lib/utils"
import type { SavedProfile } from "@/types/tab"

interface ProfileListActionsOptions {
  profiles: SavedProfile[]
  setProfiles: React.Dispatch<React.SetStateAction<SavedProfile[]>>
  setProfileGroups: React.Dispatch<React.SetStateAction<string[]>>
  refreshProfileGroups: () => Promise<void>
  confirm: ReturnType<typeof useConfirmDialog>["confirm"]
}

/** Deleting one host, and dragging hosts between groups or within one. */
export function useProfileListActions({
  profiles,
  setProfiles,
  setProfileGroups,
  refreshProfileGroups,
  confirm,
}: ProfileListActionsOptions) {
  const { t } = useTranslation()
  const { toast } = useToast()

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
    [getProfileGroupKey, profiles, refreshProfileGroups, setProfileGroups, setProfiles, t, toast]
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

  return { handleDeleteProfile, handleDragEnd }
}
