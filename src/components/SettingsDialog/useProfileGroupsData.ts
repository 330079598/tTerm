import React from "react"
import type { TFunction } from "i18next"
import { useTranslation } from "react-i18next"

import {
  UNGROUPED_KEY,
  getGroupKey,
  type GroupColumn,
} from "@/components/SettingsDialog/profileGroupDnd"
import { invokeSafe } from "@/lib/errors"
import type { SavedProfile } from "@/types/tab"

/** The profile groups and the profiles filed in them, or null after an error was shown. */
async function loadProfileGroups(t: TFunction) {
  const groupsResult = await invokeSafe<string[]>("list_profile_groups", undefined, {
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

/**
 * The groups page's data: the groups, the saved hosts, and the columns that
 * file each host under its group.
 */
export function useProfileGroupsData(refreshKey: number | undefined) {
  const { t } = useTranslation()
  const [profileGroups, setProfileGroups] = React.useState<string[]>([])
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

  return {
    profileGroups,
    setProfileGroups,
    profiles,
    setProfiles,
    refreshProfileGroups,
    groupColumns,
  }
}
