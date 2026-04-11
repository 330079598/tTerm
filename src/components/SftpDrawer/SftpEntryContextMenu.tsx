import React from "react"
import { createPortal } from "react-dom"
import { useTranslation } from "react-i18next"

import { ContextMenu } from "@/components/ContextMenu"

import type { SftpContextMenuState, SftpDirectoryEntry } from "@/components/SftpDrawer/types"

interface SftpEntryContextMenuProps {
  contextMenu: SftpContextMenuState | null
  contextMenuEntry: SftpDirectoryEntry | null
  handleDelete: () => void
  handleDownload: () => Promise<void>
  handleRename: () => void
  onClose: () => void
  selectionCount: number
}

export const SftpEntryContextMenu: React.FC<SftpEntryContextMenuProps> = ({
  contextMenu,
  contextMenuEntry,
  handleDelete,
  handleDownload,
  handleRename,
  onClose,
  selectionCount,
}) => {
  const { t } = useTranslation()

  if (!contextMenu || !contextMenuEntry) {
    return null
  }

  return createPortal(
    <ContextMenu
      x={contextMenu.x}
      y={contextMenu.y}
      actions={[
        {
          label: t("sftp.actions.download", { defaultValue: "Download" }),
          action: "download",
          icon: "copy",
          disabled: contextMenuEntry.isDir || selectionCount !== 1,
        },
        {
          label: t("sftp.actions.rename", { defaultValue: "Rename" }),
          action: "rename",
          icon: "edit",
          disabled: selectionCount !== 1,
        },
        { separator: true, label: "", action: "" },
        {
          label: t("sftp.actions.delete", { defaultValue: "Delete" }),
          action: "delete",
          icon: "x",
        },
      ]}
      onAction={(action) => {
        if (action === "download") void handleDownload()
        else if (action === "rename") handleRename()
        else if (action === "delete") handleDelete()
      }}
      onClose={onClose}
    />,
    document.body
  )
}
