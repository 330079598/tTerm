import React, { useMemo, useState } from "react"
import { File, Folder } from "lucide-react"
import {
  getIconForDirectoryPath,
  getIconForFilePath,
  getIconUrlByName,
  type MaterialIcon,
} from "vscode-material-icons"

import type { SftpDirectoryEntry } from "@/components/SftpDrawer/types"

const MATERIAL_ICONS_URL = "/assets/material-icons"

interface SftpEntryIconProps {
  entry: SftpDirectoryEntry
}

export const SftpEntryIcon: React.FC<SftpEntryIconProps> = ({ entry }) => {
  const [failedIcon, setFailedIcon] = useState<string | null>(null)

  const iconName = useMemo<MaterialIcon>(() => {
    return entry.isDir ? getIconForDirectoryPath(entry.path) : getIconForFilePath(entry.path)
  }, [entry.isDir, entry.path])
  const iconUrl = getIconUrlByName(iconName, MATERIAL_ICONS_URL)

  if (failedIcon === iconUrl) {
    return entry.isDir ? (
      <Folder className="size-4 shrink-0 text-blue-500" aria-hidden="true" />
    ) : (
      <File className="text-muted-foreground size-4 shrink-0" aria-hidden="true" />
    )
  }

  return (
    <img
      className="sftp-entry-icon"
      src={iconUrl}
      alt=""
      aria-hidden="true"
      loading="lazy"
      decoding="async"
      onError={() => setFailedIcon(iconUrl)}
    />
  )
}
