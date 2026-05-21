import React, { useCallback } from "react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"

import type { SftpDialogAction, SftpDialogState } from "@/components/SftpDrawer/types"

interface SftpDialogsProps {
  dialog: SftpDialogState
  dispatchDialog: React.Dispatch<SftpDialogAction>
  handleCommandDeleteConfirm: () => void
  handleSftpDeleteConfirm: () => void
  handleCreateDirectoryConfirm: () => void
  handleDeleteConfirm: () => void
  handleRenameConfirm: () => void
  isDeleting?: boolean
}

export const SftpDialogs: React.FC<SftpDialogsProps> = ({
  dialog,
  dispatchDialog,
  handleCommandDeleteConfirm,
  handleSftpDeleteConfirm,
  handleCreateDirectoryConfirm,
  handleDeleteConfirm,
  handleRenameConfirm,
  isDeleting = false,
}) => {
  const { t } = useTranslation()

  const closeDialog = useCallback(() => dispatchDialog({ action: "close" }), [dispatchDialog])

  const deleteEntries = dialog.type === "delete" ? dialog.entries : []
  const deleteCount = deleteEntries.length
  const folderCount = deleteEntries.filter((entry) => entry.isDir).length
  const singleEntry = deleteEntries[0] ?? null

  const commandDelete =
    dialog.type === "commandDelete"
      ? dialog
      : {
          entries: [],
          command: "",
          totalDirectories: 0,
          totalEntries: 0,
          totalFiles: 0,
          totalTruncated: false,
        }

  const renameEntry = dialog.type === "rename" ? dialog : { entry: null, newName: "" }
  const createFolder = dialog.type === "createFolder" ? dialog : { folderName: "" }

  return (
    <>
      <Dialog
        open={dialog.type === "delete"}
        onOpenChange={(open) => !open && !isDeleting && closeDialog()}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>
              {t("sftp.dialogs.deleteTitle", {
                count: deleteCount,
                defaultValue: deleteCount > 1 ? "Delete Items" : "Delete Item",
              })}
            </DialogTitle>
            <DialogDescription>
              {deleteCount > 1
                ? t("sftp.dialogs.deleteDescriptionMultiple", {
                    count: deleteCount,
                    folderCount,
                    defaultValue:
                      folderCount > 0
                        ? `Are you sure you want to delete ${deleteCount} items? ${folderCount} folder(s) will be deleted recursively. This action cannot be undone.`
                        : `Are you sure you want to delete ${deleteCount} items? This action cannot be undone.`,
                  })
                : t("sftp.dialogs.deleteDescription", {
                    defaultValue: `Are you sure you want to delete "${singleEntry?.name}"? This action cannot be undone.`,
                    name: singleEntry?.name,
                  })}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" disabled={isDeleting} onClick={closeDialog}>
              {t("common.cancel", { defaultValue: "Cancel" })}
            </Button>
            <Button variant="destructive" disabled={isDeleting} onClick={handleDeleteConfirm}>
              {t("common.delete", { defaultValue: "Delete" })}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        open={dialog.type === "commandDelete"}
        onOpenChange={(open) => !open && !isDeleting && closeDialog()}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>
              {t("sftp.dialogs.commandDeleteTitle", {
                defaultValue: "Use Command Delete?",
              })}
            </DialogTitle>
            <DialogDescription>
              {t("sftp.dialogs.commandDeleteDescription", {
                count: commandDelete.totalEntries,
                defaultValue: `Detected ${commandDelete.totalEntries}${commandDelete.totalTruncated ? "+" : ""} item(s). Review or edit the command before deleting.`,
              })}
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-3 py-3">
            <div className="text-muted-foreground text-sm">
              {t("sftp.dialogs.commandDeleteCount", {
                defaultValue: `Scanned: ${commandDelete.totalEntries}${commandDelete.totalTruncated ? "+" : ""} items, ${commandDelete.totalFiles}${commandDelete.totalTruncated ? "+" : ""} files, ${commandDelete.totalDirectories}${commandDelete.totalTruncated ? "+" : ""} folders`,
                files: commandDelete.totalTruncated
                  ? `${commandDelete.totalFiles}+`
                  : commandDelete.totalFiles,
                folders: commandDelete.totalTruncated
                  ? `${commandDelete.totalDirectories}+`
                  : commandDelete.totalDirectories,
                items: commandDelete.totalTruncated
                  ? `${commandDelete.totalEntries}+`
                  : commandDelete.totalEntries,
              })}
            </div>
            <textarea
              className="border-input bg-background ring-offset-background placeholder:text-muted-foreground focus-visible:ring-ring min-h-28 w-full rounded-md border px-3 py-2 font-mono text-sm focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-50"
              disabled={isDeleting}
              value={commandDelete.command}
              onChange={(event) =>
                dispatchDialog({
                  action: "updateCommandDeleteCommand",
                  command: event.target.value,
                })
              }
            />
          </div>
          <DialogFooter>
            <Button variant="outline" disabled={isDeleting} onClick={closeDialog}>
              {t("common.cancel", { defaultValue: "Cancel" })}
            </Button>

            <Button variant="secondary" disabled={isDeleting} onClick={handleSftpDeleteConfirm}>
              {t("sftp.dialogs.useSftpDelete", { defaultValue: "Use SFTP Delete" })}
            </Button>
            <Button
              variant="destructive"
              disabled={isDeleting}
              onClick={handleCommandDeleteConfirm}
            >
              {t("sftp.dialogs.useCommandDelete", { defaultValue: "Use Command Delete" })}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={dialog.type === "rename"} onOpenChange={(open) => !open && closeDialog()}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>
              {t("sftp.dialogs.renameTitle", { defaultValue: "Rename Item" })}
            </DialogTitle>
            <DialogDescription>
              {t("sftp.dialogs.renameDescription", {
                defaultValue: "Enter a new name for the item",
              })}
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-4 py-4">
            <div className="grid gap-2">
              <Label htmlFor="rename-input">
                {t("sftp.dialogs.nameLabel", { defaultValue: "Name" })}
              </Label>
              <Input
                id="rename-input"
                value={renameEntry.newName}
                onChange={(event) =>
                  dispatchDialog({ action: "updateRenameNewName", newName: event.target.value })
                }
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    event.preventDefault()
                    handleRenameConfirm()
                  }
                }}
                autoFocus
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={closeDialog}>
              {t("common.cancel", { defaultValue: "Cancel" })}
            </Button>
            <Button onClick={handleRenameConfirm}>
              {t("common.rename", { defaultValue: "Rename" })}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={dialog.type === "createFolder"} onOpenChange={(open) => !open && closeDialog()}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>
              {t("sftp.dialogs.createFolderTitle", { defaultValue: "Create New Folder" })}
            </DialogTitle>
            <DialogDescription>
              {t("sftp.dialogs.createFolderDescription", {
                defaultValue: "Enter a name for the new folder",
              })}
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-4 py-4">
            <div className="grid gap-2">
              <Label htmlFor="folder-name-input">
                {t("sftp.dialogs.folderNameLabel", { defaultValue: "Folder Name" })}
              </Label>
              <Input
                id="folder-name-input"
                value={createFolder.folderName}
                onChange={(event) =>
                  dispatchDialog({
                    action: "updateCreateFolderName",
                    folderName: event.target.value,
                  })
                }
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    event.preventDefault()
                    handleCreateDirectoryConfirm()
                  }
                }}
                autoFocus
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={closeDialog}>
              {t("common.cancel", { defaultValue: "Cancel" })}
            </Button>
            <Button onClick={handleCreateDirectoryConfirm}>
              {t("common.create", { defaultValue: "Create" })}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}
