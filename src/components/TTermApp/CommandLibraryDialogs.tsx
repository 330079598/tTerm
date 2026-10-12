import React from "react"

import { CommandLibrary } from "@/components/CommandLibrary"
import { CommandEditorDialog } from "@/components/CommandLibrary/CommandEditorDialog"
import type { useCommandLibraryActions } from "@/components/TTermApp/useCommandLibraryActions"

interface CommandLibraryDialogsProps {
  commandLibrary: ReturnType<typeof useCommandLibraryActions>
}

/** The command library and the editor that saves a command typed in a terminal. */
export const CommandLibraryDialogs: React.FC<CommandLibraryDialogsProps> = ({ commandLibrary }) => (
  <>
    <CommandLibrary
      open={commandLibrary.showCommandLibrary}
      onOpenChange={commandLibrary.setShowCommandLibrary}
      activeProfile={commandLibrary.activeCommandProfile}
      canInsert={commandLibrary.canInsertSavedCommand}
      onInsert={commandLibrary.handleInsertSavedCommand}
      onInsertRecent={commandLibrary.handleInsertCommandText}
      recentCommands={commandLibrary.recentCommands}
      initialSearchQuery={commandLibrary.commandLibrarySearchQuery}
    />

    <CommandEditorDialog
      open={Boolean(commandLibrary.quickSaveRequest)}
      command={null}
      draft={commandLibrary.quickSaveRequest?.draft}
      availableTags={[]}
      activeProfile={commandLibrary.quickSaveRequest?.profile}
      onOpenChange={(open) => {
        if (!open) commandLibrary.setQuickSaveRequest(null)
      }}
      onSaved={() => commandLibrary.setQuickSaveRequest(null)}
    />
  </>
)
