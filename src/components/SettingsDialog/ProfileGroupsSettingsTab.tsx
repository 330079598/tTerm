import React from "react"
import { DragDropProvider } from "@dnd-kit/react"
import { FolderTree, Trash2 } from "lucide-react"
import { useTranslation } from "react-i18next"

import { ProfileDragRow } from "@/components/SettingsDialog/ProfileDragRow"
import { ProfileGroupsToolbar } from "@/components/SettingsDialog/ProfileGroupsToolbar"
import { ProfileGroupTree } from "@/components/SettingsDialog/ProfileGroupTree"
import { SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import { useConfirmDialog } from "@/components/ui/app-dialog"
import { Button } from "@/components/ui/button"
import { ScrollArea } from "@/components/ui/scroll-area"
import type { SavedProfile, Tab } from "@/types/tab"
import { useProfileGroupActions } from "@/components/SettingsDialog/useProfileGroupActions"
import { useProfileGroupCollapse } from "@/components/SettingsDialog/useProfileGroupCollapse"
import { useProfileGroupSearch } from "@/components/SettingsDialog/useProfileGroupSearch"
import { useProfileGroupsData } from "@/components/SettingsDialog/useProfileGroupsData"
import { useProfileListActions } from "@/components/SettingsDialog/useProfileListActions"
import { useProfileSelection } from "@/components/SettingsDialog/useProfileSelection"

interface ProfileGroupsSettingsTabProps {
  onConnectProfile?: (connection: Omit<Tab, "id" | "isActive">) => void
  onEditProfile?: (profile: SavedProfile) => void
  refreshKey?: number
}

export const ProfileGroupsSettingsTab: React.FC<ProfileGroupsSettingsTabProps> = ({
  onConnectProfile,
  onEditProfile,
  refreshKey,
}) => {
  const { t } = useTranslation()
  const { confirm, ConfirmDialog } = useConfirmDialog()

  const {
    profileGroups,
    setProfileGroups,
    profiles,
    setProfiles,
    refreshProfileGroups,
    groupTree,
  } = useProfileGroupsData(refreshKey)
  const search = useProfileGroupSearch(profileGroups, profiles)
  const { collapsedKeys, toggleGroupCollapsed } = useProfileGroupCollapse(
    search.searchTree,
    search.query
  )
  const {
    selectedProfileIds,
    profileSelectionBusy,
    toggleProfileSelected,
    selectProfiles,
    clearProfileSelection,
    handleDeleteSelectedProfiles,
  } = useProfileSelection({ selectableProfiles: search.matchedProfiles, setProfiles, confirm })

  const { groupBusy, addGroup, renameGroup, moveGroup, deleteGroup } = useProfileGroupActions({
    groupTree,
    setProfileGroups,
    setProfiles,
    confirm,
  })

  const {
    handleDeleteProfile,
    handleDragStart,
    handleDragEnd,
    draggingGroupId,
    draggingProfileIds,
  } = useProfileListActions({
    profiles,
    setProfiles,
    groupTree,
    selectedProfileIds,
    moveGroup,
    refreshProfileGroups,
    confirm,
  })

  const draggedNode = draggingGroupId ? groupTree.nodes.get(draggingGroupId) : undefined
  const rootZoneVisible = (draggedNode?.depth ?? 0) > 0
  const unselectedMatchIds = search.isFiltering
    ? search.matchedProfiles
        .filter((profile) => !selectedProfileIds.has(profile.id))
        .map((profile) => profile.id)
    : []
  const nothingFound = search.isFiltering && search.matchedProfiles.length === 0

  return (
    <>
      <ScrollArea className="h-full pr-4">
        <div className="space-y-6">
          <SettingsSection
            icon={<FolderTree size={16} />}
            title={t("profileGroups.title")}
            description={t("profileGroups.pageDescription", {
              defaultValue:
                "Create groups and drag saved connection profiles between them to keep your workspace organized.",
            })}
          >
            <DragDropProvider onDragStart={handleDragStart} onDragEnd={handleDragEnd}>
              <ProfileGroupsToolbar
                groupBusy={groupBusy}
                onAddGroup={(name) => addGroup(name, null)}
                query={search.query}
                onQueryChange={search.setQuery}
                unselectedMatchIds={unselectedMatchIds}
                onSelectMatches={selectProfiles}
                rootZoneVisible={rootZoneVisible}
              />

              {selectedProfileIds.size > 0 && (
                <div className="bg-muted/40 flex items-center justify-between rounded-md border px-4 py-2">
                  <span className="text-sm font-medium">
                    {t("profiles.selectedCount", {
                      count: selectedProfileIds.size,
                      defaultValue: "{{count}} connections selected",
                    })}
                  </span>
                  <div className="flex items-center gap-2">
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      onClick={clearProfileSelection}
                      disabled={profileSelectionBusy}
                    >
                      {t("profiles.clearSelection", { defaultValue: "Clear selection" })}
                    </Button>
                    <Button
                      type="button"
                      variant="destructive"
                      size="sm"
                      onClick={handleDeleteSelectedProfiles}
                      disabled={profileSelectionBusy}
                    >
                      <Trash2 size={14} />
                      {t("profiles.deleteSelected", { defaultValue: "Delete selected" })}
                    </Button>
                  </div>
                </div>
              )}

              {nothingFound ? (
                <div className="text-muted-foreground rounded-md border border-dashed px-4 py-8 text-center">
                  <div className="text-foreground text-sm font-medium">
                    {t("profiles.searchEmpty")}
                  </div>
                  <div className="mt-1 text-xs">{t("profiles.searchEmptyDescription")}</div>
                </div>
              ) : (
                <ProfileGroupTree
                  groupTree={search.searchTree ?? groupTree}
                  draggingGroupId={draggingGroupId}
                  collapsedKeys={collapsedKeys}
                  groupBusy={groupBusy}
                  onToggleCollapsed={toggleGroupCollapsed}
                  onAddGroup={addGroup}
                  onRenameGroup={renameGroup}
                  // A search's tree counts only what it found, so the
                  // confirmation reads the group's count from the whole tree.
                  onDeleteGroup={(node) => deleteGroup(groupTree.nodes.get(node.group.id) ?? node)}
                  renderProfile={(profile, dropDisabled) => (
                    <ProfileDragRow
                      key={profile.id}
                      profile={profile}
                      dropDisabled={dropDisabled}
                      draggedCount={
                        draggingProfileIds.includes(profile.id) ? draggingProfileIds.length : 0
                      }
                      isSelected={selectedProfileIds.has(profile.id)}
                      onConnectProfile={onConnectProfile}
                      onDeleteProfile={handleDeleteProfile}
                      onEditProfile={onEditProfile}
                      onToggleSelected={toggleProfileSelected}
                    />
                  )}
                />
              )}
            </DragDropProvider>
          </SettingsSection>
        </div>
      </ScrollArea>
      <ConfirmDialog />
    </>
  )
}
