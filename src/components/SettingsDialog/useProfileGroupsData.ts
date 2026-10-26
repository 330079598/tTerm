import React from "react"
import type { TFunction } from "i18next"
import { useTranslation } from "react-i18next"

import { invokeSafe } from "@/lib/errors"
import { buildGroupTree } from "@/lib/profileGroups"
import type { ProfileGroup, SavedProfile } from "@/types/tab"

/** The profile groups and the profiles filed in them, or null after an error was shown. */
async function loadProfileGroups(t: TFunction) {
  const groupsResult = await invokeSafe<ProfileGroup[]>("list_profile_groups", undefined, {
    context: "list_profile_groups",
    title: t("errors.profileGroupsLoadFailed"),
  })
  if (!groupsResult.ok) return null

  const profilesResult = await invokeSafe<SavedProfile[]>("list_profiles", undefined, {
    context: "list_profiles",
    title: t("errors.profileGroupsLoadFailed"),
  })
  if (!profilesResult.ok) return null

  return { groups: groupsResult.value, profiles: profilesResult.value }
}

/** The groups page's data: the groups, the saved hosts, and the tree filing each host. */
export function useProfileGroupsData(refreshKey: number | undefined) {
  const { t } = useTranslation()
  const [profileGroups, setProfileGroups] = React.useState<ProfileGroup[]>([])
  const [profiles, setProfiles] = React.useState<SavedProfile[]>([])

  // The state is set in the promise callbacks rather than after an `await`,
  // which the effect lint can't yet tell apart from setting it synchronously.
  const refreshProfileGroups = React.useCallback(
    () =>
      loadProfileGroups(t).then((loaded) => {
        if (!loaded) return
        setProfileGroups(loaded.groups)
        setProfiles(loaded.profiles)
      }),
    [t]
  )

  React.useEffect(() => {
    void refreshProfileGroups()
  }, [refreshKey, refreshProfileGroups])

  const groupTree = React.useMemo(
    () => buildGroupTree(profileGroups, profiles),
    [profileGroups, profiles]
  )

  return {
    profileGroups,
    setProfileGroups,
    profiles,
    setProfiles,
    refreshProfileGroups,
    groupTree,
  }
}
