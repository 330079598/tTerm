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
  SftpCreateFolderDialogState,
  SftpDeleteDialogState,
  SftpRenameDialogState,
} from "@/components/SftpDrawer/types"

interface SftpDialogsProps {
  createFolderDialog: SftpCreateFolderDialogState
  deleteDialog: SftpDeleteDialogState
  handleCreateDirectoryConfirm: () => void
  handleDeleteConfirm: () => void
  handleRenameConfirm: () => void
  isDeleting?: boolean
  renameDialog: SftpRenameDialogState
  setCreateFolderDialog: React.Dispatch<React.SetStateAction<SftpCreateFolderDialogState>>
  setDeleteDialog: React.Dispatch<React.SetStateAction<SftpDeleteDialogState>>
  setRenameDialog: React.Dispatch<React.SetStateAction<SftpRenameDialogState>>
}

export const SftpDialogs: React.FC<SftpDialogsProps> = ({
  createFolderDialog,
  deleteDialog,
  handleCreateDirectoryConfirm,
  handleDeleteConfirm,
  handleRenameConfirm,
  isDeleting = false,
  renameDialog,
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
