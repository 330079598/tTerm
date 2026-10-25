import { useCallback } from "react"

import { useConfig } from "@/contexts/ConfigContext"
import type { ProfileSort } from "@/lib/appConfig"

/**
 * How the connections panel lists profiles. It is a setting, so it goes
 * into backups and syncs to the other devices with the rest of them.
 */
export function useProfileSort() {
  const { config, saveConfig } = useConfig()

  const changeSort = useCallback(
    (next: ProfileSort) => {
      saveConfig({ profile_sort: next }).catch(console.error)
    },
    [saveConfig]
  )

  return [config.profile_sort, changeSort] as const
}
