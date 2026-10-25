import React from "react"

import { ConnectionDialog } from "@/components/ConnectionDialog"
import { ProfilesPanel } from "@/components/ProfilesPanel"
import type { ProfileDialogState } from "@/components/TTermApp/useProfileDialogState"
import type { useProfileActions } from "@/components/TTermApp/useProfileActions"
import type { useTabOpeners } from "@/components/TTermApp/useTabOpeners"
import { Dialog, DialogContent } from "@/components/ui/dialog"
import type { RecentProfile } from "@/lib/profileRecents"

interface ProfileDialogsProps {
  dialog: ProfileDialogState
  profileActions: ReturnType<typeof useProfileActions>
  handleConnect: ReturnType<typeof useTabOpeners>["handleConnect"]
  profilesRefreshKey: number
  refreshProfiles: () => void
  showProfilesPanel: boolean
  setShowProfilesPanel: (open: boolean) => void
  collapsedGroupKeys: string[]
  onCollapsedGroupKeysChange: (groups: string[]) => void
  profileRecents: readonly RecentProfile[]
}

/** The host editor and the hosts panel the title bar opens. */
export const ProfileDialogs: React.FC<ProfileDialogsProps> = ({
  dialog,
  profileActions,
  handleConnect,
  profilesRefreshKey,
  refreshProfiles,
  showProfilesPanel,
  setShowProfilesPanel,
  collapsedGroupKeys,
  onCollapsedGroupKeysChange,
  profileRecents,
}) => (
  <>
    {dialog.showConnectionDialog && (
      <ConnectionDialog
        isOpen={dialog.showConnectionDialog}
        onClose={() => {
          dialog.setShowConnectionDialog(false)
          dialog.setEditingProfile(null)
          dialog.setEditingProfileTabId(null)
          dialog.setDuplicatingProfile(null)
          dialog.setDraftProfile(null)
          dialog.setSavingQuickConnectTabId(null)
          refreshProfiles()
        }}
        onConnect={handleConnect}
        editProfile={dialog.editingProfile}
        duplicateProfile={dialog.duplicatingProfile}
        draftProfile={dialog.draftProfile}
        saveOnly={dialog.savingQuickConnectTabId !== null}
        onSaved={profileActions.handleProfileSaved}
        typedPasswordTabId={dialog.savingQuickConnectTabId ?? undefined}
        canReconnect={dialog.editingProfileTabId !== null}
      />
    )}

    <Dialog open={showProfilesPanel} onOpenChange={setShowProfilesPanel}>
      <DialogContent
        showCloseButton={false}
        className="flex h-[min(720px,85vh)] flex-col overflow-hidden border-0 p-0 shadow-none sm:max-w-3xl"
      >
        <ProfilesPanel
          refreshKey={profilesRefreshKey}
          collapsedGroupKeys={collapsedGroupKeys}
          profileRecents={profileRecents}
          surface="panel"
          onCollapsedGroupKeysChange={onCollapsedGroupKeysChange}
          onClose={() => setShowProfilesPanel(false)}
          onCreate={() => {
            dialog.setEditingProfile(null)
            dialog.setDuplicatingProfile(null)
            dialog.setShowConnectionDialog(true)
            setShowProfilesPanel(false)
          }}
          onConnect={(connection) => {
            handleConnect(connection)
            setShowProfilesPanel(false)
          }}
          onEdit={(profile) => {
            profileActions.handleEditProfile(profile)
            setShowProfilesPanel(false)
          }}
          onDuplicate={(profile) => {
            profileActions.handleDuplicateProfile(profile)
            setShowProfilesPanel(false)
          }}
          onDeleteProfile={profileActions.handleDeleteProfile}
        />
      </DialogContent>
    </Dialog>
  </>
)
