import React from "react"
import { useTranslation } from "react-i18next"

import { UNGROUPED_KEY } from "@/components/SettingsDialog/profileGroupDnd"
import { useConfig } from "@/contexts/ConfigContext"
import { useToast } from "@/hooks/use-toast"
import { emptyGroupIds, type GroupTree } from "@/lib/profileGroups"
import { toErrorMessage } from "@/lib/utils"

/**
 * Which group cards are folded. Without a search they fold as the user left
 * them, which is saved. A search folds the groups it found nothing in and
 * opens the rest; a card the user folds or opens then stays so until the
 * search changes, and that isn't saved, so clearing the search brings the
 * saved folds back.
 */
export function useProfileGroupCollapse(searchTree: GroupTree | null, query: string) {
  const { t } = useTranslation()
  const { toast } = useToast()
  const { appState, saveAppState } = useConfig()
  const savedKeys = React.useMemo(
    () => new Set(appState.collapsedProfileGroupKeys),
    [appState.collapsedProfileGroupKeys]
  )
  const [searchToggles, setSearchToggles] = React.useState<{
    query: string
    keys: ReadonlySet<string>
  }>({ query: "", keys: new Set() })

  const collapsedKeys: ReadonlySet<string> = React.useMemo(() => {
    if (!searchTree) return savedKeys
    const folded = emptyGroupIds(searchTree)
    if (searchTree.ungrouped.length === 0) folded.add(UNGROUPED_KEY)
    if (searchToggles.query === query) {
      for (const key of searchToggles.keys) {
        if (!folded.delete(key)) folded.add(key)
      }
    }
    return folded
  }, [query, savedKeys, searchToggles, searchTree])

  const toggleGroupCollapsed = React.useCallback(
    (groupKey: string) => {
      if (searchTree) {
        setSearchToggles((current) => {
          const keys = new Set(current.query === query ? current.keys : [])
          if (!keys.delete(groupKey)) keys.add(groupKey)
          return { query, keys }
        })
        return
      }

      const next = new Set(savedKeys)
      if (!next.delete(groupKey)) next.add(groupKey)
      saveAppState({ collapsedProfileGroupKeys: Array.from(next) }).catch((error) => {
        toast({
          title: t("settings.saveFailed", { defaultValue: "Failed to save settings" }),
          description: toErrorMessage(error),
          variant: "destructive",
        })
      })
    },
    [query, saveAppState, savedKeys, searchTree, t, toast]
  )

  return { collapsedKeys, toggleGroupCollapsed }
}
