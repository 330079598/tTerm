import React from "react"
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

import type {
  SftpCommandDeleteDialogState,
  SftpCreateFolderDialogState,
  SftpDeleteDialogState,
  SftpRenameDialogState,
} from "@/components/SftpDrawer/types"

interface SftpDialogsProps {
  commandDeleteDialog: SftpCommandDeleteDialogState
  createFolderDialog: SftpCreateFolderDialogState
  deleteDialog: SftpDeleteDialogState
  handleCommandDeleteConfirm: () => void
  handleSftpDeleteConfirm: () => void
  handleCreateDirectoryConfirm: () => void
  handleDeleteConfirm: () => void
  handleRenameConfirm: () => void
  isDeleting?: boolean
  renameDialog: SftpRenameDialogState
  setCommandDeleteDialog: React.Dispatch<React.SetStateAction<SftpCommandDeleteDialogState>>
  setCreateFolderDialog: React.Dispatch<React.SetStateAction<SftpCreateFolderDialogState>>
  setDeleteDialog: React.Dispatch<React.SetStateAction<SftpDeleteDialogState>>
  setRenameDialog: React.Dispatch<React.SetStateAction<SftpRenameDialogState>>
}

export const SftpDialogs: React.FC<SftpDialogsProps> = ({
  commandDeleteDialog,
  createFolderDialog,
  deleteDialog,
  handleCommandDeleteConfirm,
  handleSftpDeleteConfirm,
  handleCreateDirectoryConfirm,
  handleDeleteConfirm,
  handleRenameConfirm,
  isDeleting = false,
  renameDialog,
  setCommandDeleteDialog,
  setCreateFolderDialog,
  setDeleteDialog,
  setRenameDialog,
}) => {
  const { t } = useTranslation()
  const deleteCount = deleteDialog.entries.length
  const folderCount = deleteDialog.entries.filter((entry) => entry.isDir).length
  const singleEntry = deleteDialog.entries[0] ?? null

  return (
    <>
      <Dialog
        open={deleteDialog.open}
        onOpenChange={(open) =>
          !open && !isDeleting && setDeleteDialog({ open: false, entries: [] })
        }
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
            <Button
              variant="outline"
              disabled={isDeleting}
              onClick={() => setDeleteDialog({ open: false, entries: [] })}
            >
              {t("common.cancel", { defaultValue: "Cancel" })}
            </Button>
            <Button variant="destructive" disabled={isDeleting} onClick={handleDeleteConfirm}>
              {t("common.delete", { defaultValue: "Delete" })}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        open={commandDeleteDialog.open}
        onOpenChange={(open) =>
          !open &&
          !isDeleting &&
          setCommandDeleteDialog({
            command: "",
            entries: [],
            open: false,
            totalDirectories: 0,
            totalEntries: 0,
            totalFiles: 0,
            totalTruncated: false,
          })
        }
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
                count: commandDeleteDialog.totalEntries,
                defaultValue: `Detected ${commandDeleteDialog.totalEntries}${commandDeleteDialog.totalTruncated ? "+" : ""} item(s). Review or edit the command before deleting.`,
              })}
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-3 py-3">
            <div className="text-muted-foreground text-sm">
              {t("sftp.dialogs.commandDeleteCount", {
                defaultValue: `Scanned: ${commandDeleteDialog.totalEntries}${commandDeleteDialog.totalTruncated ? "+" : ""} items, ${commandDeleteDialog.totalFiles}${commandDeleteDialog.totalTruncated ? "+" : ""} files, ${commandDeleteDialog.totalDirectories}${commandDeleteDialog.totalTruncated ? "+" : ""} folders`,
                files: commandDeleteDialog.totalTruncated
                  ? `${commandDeleteDialog.totalFiles}+`
                  : commandDeleteDialog.totalFiles,
                folders: commandDeleteDialog.totalTruncated
                  ? `${commandDeleteDialog.totalDirectories}+`
                  : commandDeleteDialog.totalDirectories,
                items: commandDeleteDialog.totalTruncated
                  ? `${commandDeleteDialog.totalEntries}+`
                  : commandDeleteDialog.totalEntries,
              })}
            </div>
            <textarea
              className="border-input bg-background ring-offset-background placeholder:text-muted-foreground focus-visible:ring-ring min-h-28 w-full rounded-md border px-3 py-2 font-mono text-sm focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-50"
              disabled={isDeleting}
              value={commandDeleteDialog.command}
              onChange={(event) =>
                setCommandDeleteDialog((current) => ({
                  ...current,
                  command: event.target.value,
                }))
              }
            />
          </div>
          <DialogFooter>
            <Button
              variant="outline"
              disabled={isDeleting}
              onClick={() =>
                setCommandDeleteDialog({
                  command: "",
                  entries: [],
                  open: false,
                  totalDirectories: 0,
                  totalEntries: 0,
                  totalFiles: 0,
                  totalTruncated: false,
                })
              }
            >
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

      <Dialog
        open={renameDialog.open}
        onOpenChange={(open) => !open && setRenameDialog({ open: false, entry: null, newName: "" })}
      >
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
                value={renameDialog.newName}
                onChange={(event) =>
                  setRenameDialog({ ...renameDialog, newName: event.target.value })
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
            <Button
              variant="outline"
              onClick={() => setRenameDialog({ open: false, entry: null, newName: "" })}
            >
              {t("common.cancel", { defaultValue: "Cancel" })}
            </Button>
            <Button onClick={handleRenameConfirm}>
              {t("common.rename", { defaultValue: "Rename" })}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        open={createFolderDialog.open}
        onOpenChange={(open) => !open && setCreateFolderDialog({ open: false, folderName: "" })}
      >
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
                value={createFolderDialog.folderName}
                onChange={(event) =>
                  setCreateFolderDialog({ ...createFolderDialog, folderName: event.target.value })
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
            <Button
              variant="outline"
              onClick={() => setCreateFolderDialog({ open: false, folderName: "" })}
            >
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
