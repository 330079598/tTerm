import type { SftpDialogAction, SftpDialogState } from "@/components/SftpDrawer/types"

export function sftpDialogReducer(
  state: SftpDialogState,
  action: SftpDialogAction
): SftpDialogState {
  switch (action.action) {
    case "close":
      return { type: "none" }
    case "openDelete":
      return { type: "delete", entries: action.entries }
    case "openCommandDelete":
      return {
        type: "commandDelete",
        entries: action.entries,
        command: action.command,
        totalDirectories: action.totalDirectories,
        totalEntries: action.totalEntries,
        totalFiles: action.totalFiles,
        totalTruncated: action.totalTruncated,
      }
    case "openRename":
      return { type: "rename", entry: action.entry, newName: action.newName }
    case "openCreateFolder":
      return { type: "createFolder", folderName: "" }
    case "updateRenameNewName":
      if (state.type !== "rename") return state
      return { ...state, newName: action.newName }
    case "updateCreateFolderName":
      if (state.type !== "createFolder") return state
      return { ...state, folderName: action.folderName }
    case "updateCommandDeleteCommand":
      if (state.type !== "commandDelete") return state
      return { ...state, command: action.command }
    default:
      return state
  }
}
