import React from "react"
import { invoke } from "@tauri-apps/api/core"
import { useTranslation } from "react-i18next"

import { profileTunnelNote } from "@/components/SettingsDialog/profileDeletion"
import { findTunnelsUsingProfile } from "@/components/TunnelsPanel/profileTunnels"
import type { useConfirmDialog } from "@/components/ui/app-dialog"
import { useToast } from "@/hooks/use-toast"
import { toErrorMessage } from "@/lib/utils"
import type { SavedProfile } from "@/types/tab"

interface ProfileSelectionOptions {
  profiles: SavedProfile[]
  setProfiles: React.Dispatch<React.SetStateAction<SavedProfile[]>>
  confirm: ReturnType<typeof useConfirmDialog>["confirm"]
}

/** The hosts ticked on the groups page, and deleting them together. */
export function useProfileSelection({ profiles, setProfiles, confirm }: ProfileSelectionOptions) {
  const { t } = useTranslation()
  const { toast } = useToast()
  const [selectedProfileIds, setSelectedProfileIds] = React.useState<Set<string>>(new Set())
  const [profileSelectionBusy, setProfileSelectionBusy] = React.useState(false)

  const [selectionProfiles, setSelectionProfiles] = React.useState(profiles)
  if (selectionProfiles !== profiles) {
    setSelectionProfiles(profiles)
    const knownIds = new Set(profiles.map((profile) => profile.id))
    setSelectedProfileIds((current) => {
      const next = new Set([...current].filter((id) => knownIds.has(id)))
      return next.size === current.size ? current : next
    })
  }

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
    const tunnelNote = profileTunnelNote(t, tunnels)

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
  }, [confirm, selectedProfileIds, setProfiles, t, toast])

  return {
    selectedProfileIds,
    profileSelectionBusy,
    toggleProfileSelected,
    clearProfileSelection,
    handleDeleteSelectedProfiles,
  }
}
