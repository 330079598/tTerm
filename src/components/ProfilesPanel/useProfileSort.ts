import { useCallback, useState } from "react"

import {
  loadProfileSort,
  saveProfileSort,
  type ProfileSort,
} from "@/components/ProfilesPanel/profileListing"

/** How the connections panel lists profiles, remembered on this device. */
export function useProfileSort() {
  const [sort, setSort] = useState<ProfileSort>(() =>
    typeof window === "undefined" ? "default" : loadProfileSort(window.localStorage)
  )

  const changeSort = useCallback((next: ProfileSort) => {
    setSort(next)
    saveProfileSort(window.localStorage, next)
  }, [])

  return [sort, changeSort] as const
}
