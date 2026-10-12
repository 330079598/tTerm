import { useState } from "react"

import { SavedProfile } from "@/components/ProfilesPanel"

/**
 * The profile dialog's state: whether it is open, and the profile it
 * creates, edits or duplicates.
 */
export function useProfileDialogState() {
  const [showConnectionDialog, setShowConnectionDialog] = useState(false)
  // Starting values of a new profile, and the quick connect tab it is saved for.
  const [draftProfile, setDraftProfile] = useState<SavedProfile | null>(null)
  const [savingQuickConnectTabId, setSavingQuickConnectTabId] = useState<string | null>(null)
  const [editingProfile, setEditingProfile] = useState<SavedProfile | null>(null)
  // The tab whose profile is being edited, which can be reconnected once it is saved.
  const [editingProfileTabId, setEditingProfileTabId] = useState<string | null>(null)
  const [duplicatingProfile, setDuplicatingProfile] = useState<SavedProfile | null>(null)

  return {
    showConnectionDialog,
    setShowConnectionDialog,
    draftProfile,
    setDraftProfile,
    savingQuickConnectTabId,
    setSavingQuickConnectTabId,
    editingProfile,
    setEditingProfile,
    editingProfileTabId,
    setEditingProfileTabId,
    duplicatingProfile,
    setDuplicatingProfile,
  }
}

export type ProfileDialogState = ReturnType<typeof useProfileDialogState>
