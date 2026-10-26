import { useEffect, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { useTranslation } from "react-i18next"

import { useToast } from "@/hooks/use-toast"
import type { ProfileGroup, SavedProfile } from "@/types/tab"

/** The saved profiles and their groups, loaded again whenever `refreshKey` changes. */
export function useProfilesPanelData(refreshKey: number | undefined) {
  const { t } = useTranslation()
  const { toast } = useToast()
  const [profiles, setProfiles] = useState<SavedProfile[]>([])
  const [groups, setGroups] = useState<ProfileGroup[]>([])
  const [isLoading, setIsLoading] = useState(true)

  const [loadingRefreshKey, setLoadingRefreshKey] = useState(refreshKey)
  if (loadingRefreshKey !== refreshKey) {
    setLoadingRefreshKey(refreshKey)
    setIsLoading(true)
  }

  useEffect(() => {
    Promise.all([
      invoke<SavedProfile[]>("list_profiles"),
      invoke<ProfileGroup[]>("list_profile_groups"),
    ])
      .then(([loadedProfiles, loadedGroups]) => {
        setProfiles(loadedProfiles)
        setGroups(loadedGroups)
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

  return { profiles, setProfiles, groups, isLoading }
}
