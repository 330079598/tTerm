import React from "react"
import { DragDropProvider } from "@dnd-kit/react"
import { FolderTree, Plus, Trash2 } from "lucide-react"
import { useTranslation } from "react-i18next"

import { RootDropZone } from "@/components/SettingsDialog/ProfileGroupCard"
import { ProfileDragRow } from "@/components/SettingsDialog/ProfileDragRow"
import { ProfileGroupTree } from "@/components/SettingsDialog/ProfileGroupTree"
import { SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import { useConfirmDialog } from "@/components/ui/app-dialog"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { ScrollArea } from "@/components/ui/scroll-area"
import { useConfig } from "@/contexts/ConfigContext"
import { useToast } from "@/hooks/use-toast"
import { isImeKeyEvent } from "@/lib/ime"
import { toErrorMessage } from "@/lib/utils"
import type { SavedProfile, Tab } from "@/types/tab"
import { useProfileGroupActions } from "@/components/SettingsDialog/useProfileGroupActions"
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
  const { toast } = useToast()
  const { appState, saveAppState } = useConfig()
  const { confirm, ConfirmDialog } = useConfirmDialog()
  const [newGroupName, setNewGroupName] = React.useState("")
  const collapsedGroupKeySet = React.useMemo(
    () => new Set(appState.collapsedProfileGroupKeys),
    [appState.collapsedProfileGroupKeys]
  )

  const { setProfileGroups, profiles, setProfiles, refreshProfileGroups, groupTree } =
    useProfileGroupsData(refreshKey)
  const {
    selectedProfileIds,
    profileSelectionBusy,
    toggleProfileSelected,
    clearProfileSelection,
    handleDeleteSelectedProfiles,
  } = useProfileSelection({ profiles, setProfiles, confirm })

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

  const handleAddGroup = async () => {
    if (await addGroup(newGroupName, null)) setNewGroupName("")
  }

  const toggleGroupCollapsed = React.useCallback(
    (groupKey: string) => {
      const next = new Set(collapsedGroupKeySet)
      if (next.has(groupKey)) {
        next.delete(groupKey)
      } else {
        next.add(groupKey)
      }

      saveAppState({ collapsedProfileGroupKeys: Array.from(next) }).catch((error) => {
        toast({
          title: t("settings.saveFailed", { defaultValue: "Failed to save settings" }),
          description: toErrorMessage(error),
          variant: "destructive",
        })
      })
    },
    [collapsedGroupKeySet, saveAppState, t, toast]
  )

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
              <div className="relative">
                <div className="bg-card rounded-md border p-4">
                  <div className="flex max-w-lg gap-2">
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
                </div>
                {rootZoneVisible && <RootDropZone />}
              </div>

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

              <ProfileGroupTree
                groupTree={groupTree}
                draggingGroupId={draggingGroupId}
                collapsedKeys={collapsedGroupKeySet}
                groupBusy={groupBusy}
                onToggleCollapsed={toggleGroupCollapsed}
                onAddGroup={addGroup}
                onRenameGroup={renameGroup}
                onDeleteGroup={deleteGroup}
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
            </DragDropProvider>
          </SettingsSection>
        </div>
      </ScrollArea>
      <ConfirmDialog />
    </>
  )
}
