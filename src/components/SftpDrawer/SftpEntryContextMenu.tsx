import React from "react"
import { createPortal } from "react-dom"
import { useTranslation } from "react-i18next"

import { ContextMenu } from "@/components/ContextMenu"
import { archiveFormatOf } from "@/components/SftpDrawer/sftpArchive"
import { editorDisplayName } from "@/lib/externalEditor"

import type { SftpContextMenuState, SftpDirectoryEntry } from "@/components/SftpDrawer/types"

interface SftpEntryContextMenuProps {
  contextMenu: SftpContextMenuState | null
  contextMenuEntry: SftpDirectoryEntry | null
  /** Missing when the drawer's tab has no shell to send `cd` to. */
  handleCdInTerminal?: (path: string) => void
  handleCompress: () => void
  handleCopyPath: () => Promise<void>
  handleDelete: () => void
  handleDownload: () => Promise<void>
  handleEdit: () => void
  /** Opens the file in its default program, or in the external editor. */
  handleOpenExternally: (inEditor: boolean) => void
  /** Asks which program to open the file in. */
  handleOpenWith: () => void
  /** The external editor's program; empty until one is picked. */
  externalEditor: string
  handleExtract: () => void
  handlePermissions: () => void
  handleRename: () => void
  /** Set while the rows are search results, which can be shown in their folder. */
  handleReveal?: () => Promise<void>
  isDeleting: boolean
  onClose: () => void
  selectionCount: number
}

export const SftpEntryContextMenu: React.FC<SftpEntryContextMenuProps> = ({
  contextMenu,
  contextMenuEntry,
  handleCdInTerminal,
  handleCompress,
  handleCopyPath,
  handleDelete,
  handleDownload,
  handleEdit,
  handleOpenExternally,
  handleOpenWith,
  externalEditor,
  handleExtract,
  handlePermissions,
  handleRename,
  handleReveal,
  isDeleting,
  onClose,
  selectionCount,
}) => {
  const { t } = useTranslation()

  if (!contextMenu || !contextMenuEntry) {
    return null
  }
  const canExtract = selectionCount === 1 && archiveFormatOf(contextMenuEntry) !== null
  const canOpen = selectionCount === 1 && !contextMenuEntry.isDir

  return createPortal(
    <ContextMenu
      x={contextMenu.x}
      y={contextMenu.y}
      actions={[
        {
          label: t("sftp.actions.copyPath", { defaultValue: "Copy Full Path" }),
          action: "copy-path",
          icon: "copy",
        },
        ...(handleReveal
          ? [{ label: t("sftp.actions.showInFolder"), action: "reveal", icon: "folder" }]
          : []),
        ...(handleCdInTerminal && contextMenuEntry.isDir
          ? [
              {
                label: t("sftp.actions.openInTerminal"),
                action: "cd",
                icon: "terminal",
              },
            ]
          : []),
        {
          label:
            selectionCount > 1
              ? t("sftp.actions.downloadSelected", { defaultValue: "Download Selection" })
              : contextMenuEntry.isDir
                ? t("sftp.actions.downloadFolder", { defaultValue: "Download Folder" })
                : t("sftp.actions.download", { defaultValue: "Download" }),
          action: "download",
          icon:
            selectionCount > 1 ? "download" : contextMenuEntry.isDir ? "folder-down" : "file-down",
        },
        {
          label: t("sftp.actions.compress"),
          action: "compress",
          icon: "archive",
        },
        ...(canExtract
          ? [{ label: t("sftp.actions.extract"), action: "extract", icon: "extract" }]
          : []),
        {
          label: t("sftp.actions.edit", { defaultValue: "Edit" }),
          action: "edit",
          icon: "edit",
          disabled: !canOpen,
        },
        {
          label: externalEditor
            ? t("sftp.actions.openInProgram", { program: editorDisplayName(externalEditor) })
            : t("sftp.actions.openInEditor"),
          action: "open-editor",
          icon: "file-pen",
          disabled: !canOpen,
        },
        {
          label: t("sftp.actions.openWithDefault"),
          action: "open-default",
          icon: "external-link",
          disabled: !canOpen,
        },
        {
          label: t("sftp.actions.openWith"),
          action: "open-with",
          icon: "app-window",
          disabled: !canOpen,
        },
        {
          label: t("sftp.actions.rename", { defaultValue: "Rename" }),
          action: "rename",
          icon: "edit",
          disabled: selectionCount !== 1,
        },
        {
          label: t("sftp.actions.permissions", { defaultValue: "Permissions…" }),
          action: "permissions",
          icon: "edit",
        },
        { separator: true, label: "", action: "" },
        {
          label: t("sftp.actions.delete", { defaultValue: "Delete" }),
          action: "delete",
          icon: "x",
          disabled: isDeleting,
        },
      ]}
      onAction={(action) => {
        if (action === "download") void handleDownload()
        else if (action === "copy-path") void handleCopyPath()
        else if (action === "cd") handleCdInTerminal?.(contextMenuEntry.path)
        else if (action === "reveal") void handleReveal?.()
        else if (action === "compress") handleCompress()
        else if (action === "extract") handleExtract()
        else if (action === "edit") handleEdit()
        else if (action === "open-editor") handleOpenExternally(true)
        else if (action === "open-default") handleOpenExternally(false)
        else if (action === "open-with") handleOpenWith()
        else if (action === "rename") handleRename()
        else if (action === "permissions") handlePermissions()
        else if (action === "delete") handleDelete()
      }}
      onClose={onClose}
    />,
    document.body
  )
}
