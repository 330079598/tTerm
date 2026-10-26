import React from "react"
import { CheckCheck, Plus, Search } from "lucide-react"
import { useTranslation } from "react-i18next"

import { RootDropZone } from "@/components/SettingsDialog/ProfileGroupCard"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { isImeKeyEvent } from "@/lib/ime"

interface ProfileGroupsToolbarProps {
  groupBusy: boolean
  onAddGroup: (name: string) => Promise<boolean>
  query: string
  onQueryChange: (query: string) => void
  /** The hosts a search found that aren't ticked yet; none without a search. */
  unselectedMatchIds: readonly string[]
  onSelectMatches: (profileIds: readonly string[]) => void
  /** Covers the bar with the drop zone that moves a dragged group to the top. */
  rootZoneVisible: boolean
}

/** The bar above the groups tree: a new top-level group, and the search. */
export const ProfileGroupsToolbar: React.FC<ProfileGroupsToolbarProps> = ({
  groupBusy,
  onAddGroup,
  query,
  onQueryChange,
  unselectedMatchIds,
  onSelectMatches,
  rootZoneVisible,
}) => {
  const { t } = useTranslation()
  const [newGroupName, setNewGroupName] = React.useState("")

  const handleAddGroup = async () => {
    if (await onAddGroup(newGroupName)) setNewGroupName("")
  }

  return (
    <div className="relative">
      <div className="bg-card flex flex-wrap items-center gap-2 rounded-md border p-4">
        <div className="flex min-w-60 flex-1 gap-2">
          <Input
            value={newGroupName}
            onChange={(event) => setNewGroupName(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !isImeKeyEvent(event.nativeEvent)) {
                event.preventDefault()
                void handleAddGroup()
              }
            }}
            placeholder={t("profileGroups.namePlaceholder")}
            disabled={groupBusy}
            className="h-8"
          />
          <Button
            type="button"
            size="sm"
            onClick={handleAddGroup}
            disabled={groupBusy || !newGroupName.trim()}
          >
            <Plus size={14} />
            {t("profileGroups.addButton")}
          </Button>
        </div>
        <div className="flex min-w-60 flex-1 items-center gap-2">
          <div className="relative min-w-0 flex-1">
            <Search className="text-muted-foreground pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2" />
            <Input
              type="search"
              value={query}
              onChange={(event) => onQueryChange(event.target.value)}
              placeholder={t("profiles.searchPlaceholder")}
              aria-label={t("profileGroups.search", { defaultValue: "Search connections" })}
              className="h-8 pl-8"
            />
          </div>
          {query.trim() && (
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => onSelectMatches(unselectedMatchIds)}
              disabled={unselectedMatchIds.length === 0}
            >
              <CheckCheck size={14} />
              {t("profileGroups.selectMatches", { defaultValue: "Select all" })}
            </Button>
          )}
        </div>
      </div>
      {rootZoneVisible && <RootDropZone />}
    </div>
  )
}
