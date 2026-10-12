import type { TFunction } from "i18next"
import React from "react"
import { invoke } from "@tauri-apps/api/core"

import type { useConfirmDialog } from "@/components/ui/app-dialog"
import type { useToast } from "@/hooks/use-toast"
import type { SavedProfile } from "@/types/tab"

interface ProfileGroupActionsOptions {
  newGroupName: string
  setNewGroupName: React.Dispatch<React.SetStateAction<string>>
  profileGroups: string[]
  setProfileGroups: React.Dispatch<React.SetStateAction<string[]>>
  profiles: SavedProfile[]
  setProfiles: React.Dispatch<React.SetStateAction<SavedProfile[]>>
  editingGroupDraft: string
  setEditingGroupName: React.Dispatch<React.SetStateAction<string | null>>
  setEditingGroupDraft: React.Dispatch<React.SetStateAction<string>>
  setGroupBusy: React.Dispatch<React.SetStateAction<boolean>>
  showGroupError: (error: unknown) => void
  confirm: ReturnType<typeof useConfirmDialog>["confirm"]
  toast: ReturnType<typeof useToast>["toast"]
  t: TFunction
}

/** Adding, renaming and deleting profile groups. */
export function useProfileGroupActions({
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
}: ProfileGroupActionsOptions) {
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
  }, [
    newGroupName,
    profileGroups,
    showGroupError,
    t,
    toast,
    setGroupBusy,
    setNewGroupName,
    setProfileGroups,
  ])

  const startEditGroup = React.useCallback(
    (group: string) => {
      setEditingGroupName(group)
      setEditingGroupDraft(group)
    },
    [setEditingGroupDraft, setEditingGroupName]
  )

  const cancelEditGroup = React.useCallback(() => {
    setEditingGroupName(null)
    setEditingGroupDraft("")
  }, [setEditingGroupDraft, setEditingGroupName])

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
    [
      cancelEditGroup,
      editingGroupDraft,
      profileGroups,
      showGroupError,
      t,
      toast,
      setGroupBusy,
      setProfileGroups,
      setProfiles,
    ]
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
    [confirm, profiles, showGroupError, t, setGroupBusy, setProfileGroups, setProfiles]
  )

  return { handleAddGroup, startEditGroup, cancelEditGroup, handleRenameGroup, handleDeleteGroup }
}
