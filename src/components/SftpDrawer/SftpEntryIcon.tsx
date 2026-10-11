import React, { useEffect, useMemo, useState } from "react"
import { File, Folder } from "lucide-react"

import type { SftpDirectoryEntry } from "@/components/SftpDrawer/types"

const MATERIAL_ICONS_URL = "/assets/material-icons"

type MaterialIcons = typeof import("vscode-material-icons")

// The icon manifest is a few hundred KB; load it with the drawer instead of
// with the app.
let materialIcons: MaterialIcons | null = null
let materialIconsPromise: Promise<MaterialIcons> | null = null

export function preloadMaterialIcons(): Promise<MaterialIcons> {
  materialIconsPromise ??= import("vscode-material-icons").then(
    (module) => (materialIcons = module),
    (error: unknown) => {
      materialIconsPromise = null
      throw error
    }
  )
  return materialIconsPromise
}

function useMaterialIcons(): MaterialIcons | null {
  const [icons, setIcons] = useState(materialIcons)

  useEffect(() => {
    if (icons) return
    let cancelled = false
    preloadMaterialIcons().then((loaded) => {
      if (!cancelled) setIcons(loaded)
    }, console.error)
    return () => {
      cancelled = true
    }
  }, [icons])

  return icons
}

interface SftpEntryIconProps {
  entry: SftpDirectoryEntry
}

export const SftpEntryIcon: React.FC<SftpEntryIconProps> = ({ entry }) => {
  const [failedIcon, setFailedIcon] = useState<string | null>(null)
  const icons = useMaterialIcons()

  const iconUrl = useMemo(() => {
    if (!icons) return null
    const iconName = entry.isDir
      ? icons.getIconForDirectoryPath(entry.path)
      : icons.getIconForFilePath(entry.path)
    return icons.getIconUrlByName(iconName, MATERIAL_ICONS_URL)
  }, [icons, entry.isDir, entry.path])

  if (!iconUrl || failedIcon === iconUrl) {
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
