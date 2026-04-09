import React from "react"
import {
  AlertCircle,
  ArrowUpFromLine,
  File,
  Folder,
  FolderPlus,
  Loader2,
  RefreshCcw,
} from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { ScrollArea } from "@/components/ui/scroll-area"
import { cn } from "@/lib/utils"

import { formatBytes, formatTimestamp } from "@/components/SftpDrawer/sftpDrawerUtils"
import type {
  SftpContextMenuState,
  SftpDirectoryEntry,
  SftpDirectoryListing,
} from "@/components/SftpDrawer/types"

interface SftpDrawerContentProps {
  error: string | null
  handleDragEnter: (event: React.DragEvent<HTMLDivElement>) => void
  handleDragLeave: (event: React.DragEvent<HTMLDivElement>) => void
  handleDragOver: (event: React.DragEvent<HTMLDivElement>) => void
  handleDrop: (event: React.DragEvent<HTMLDivElement>) => Promise<void>
  handleOpenEntry: (entry: SftpDirectoryEntry) => void | Promise<void>
  isDragActive: boolean
  isLoading: boolean
  listing: SftpDirectoryListing | null
  loadDirectory: (path?: string | null) => Promise<void>
  selectedPath: string | null
  setContextMenu: React.Dispatch<React.SetStateAction<SftpContextMenuState | null>>
  setSelectedPath: React.Dispatch<React.SetStateAction<string | null>>
}

export const SftpDrawerContent: React.FC<SftpDrawerContentProps> = ({
  error,
  handleDragEnter,
  handleDragLeave,
  handleDragOver,
  handleDrop,
  handleOpenEntry,
  isDragActive,
  isLoading,
  listing,
  loadDirectory,
  selectedPath,
  setContextMenu,
  setSelectedPath,
}) => {
  const { t } = useTranslation()

  return (
    <div
      className={cn("sftp-drawer-body", isDragActive && "drag-active")}
      onDragEnter={handleDragEnter}
      onDragLeave={handleDragLeave}
      onDragOver={handleDragOver}
      onDrop={(event) => void handleDrop(event)}
    >
      {isDragActive && (
        <div className="sftp-drag-overlay">
          <div className="sftp-drag-hint">
            <ArrowUpFromLine className="sftp-drag-hint-icon" />
            <div className="text-center">
              <p className="sftp-drag-hint-title">
                {t("sftp.dropFiles", { defaultValue: "Drop files to upload" })}
              </p>
              <p className="sftp-drag-hint-description">
                {t("sftp.dropFilesHint", {
                  defaultValue: "Files will be uploaded to current directory",
                })}
              </p>
            </div>
          </div>
        </div>
      )}

      <div className="sftp-table-shell">
        <ScrollArea className="flex-1">
          {isLoading && (
            <div className="text-muted-foreground flex min-h-[200px] flex-col items-center justify-center gap-3">
              <Loader2 className="size-6 animate-spin" />
              <span className="text-sm">{t("sftp.loading", { defaultValue: "Loading..." })}</span>
            </div>
          )}

          {!isLoading && error && (
            <div className="text-destructive flex min-h-[200px] flex-col items-center justify-center gap-3">
              <AlertCircle className="size-6" />
              <span className="text-sm">{error}</span>
              <Button
                variant="outline"
                size="sm"
                onClick={() => void loadDirectory(listing?.currentPath ?? null)}
                className="mt-2"
              >
                <RefreshCcw className="size-4" />
                {t("sftp.retry", { defaultValue: "Retry" })}
              </Button>
            </div>
          )}

          {!isLoading &&
            !error &&
            listing?.entries.map((entry) => {
              const isSelected = entry.path === selectedPath
              return (
                <button
                  key={entry.path}
                  type="button"
                  className={cn("sftp-row", isSelected && "sftp-row-selected")}
                  aria-pressed={isSelected}
                  onClick={() => {
                    setSelectedPath(entry.path)
                    setContextMenu(null)
                  }}
                  onDoubleClick={() => void handleOpenEntry(entry)}
                  onContextMenu={(event) => {
                    event.preventDefault()
                    setSelectedPath(entry.path)
                    setContextMenu({ x: event.clientX, y: event.clientY, entryPath: entry.path })
                  }}
                >
                  <span className="sftp-cell sftp-name-cell">
                    {entry.isDir ? (
                      <Folder className="size-4 shrink-0 text-blue-500" />
                    ) : (
                      <File className="text-muted-foreground size-4 shrink-0" />
                    )}
                    <span className="truncate">{entry.name}</span>
                  </span>
                  <span className="sftp-cell">{formatTimestamp(entry.modifiedAt)}</span>
                  <span className="sftp-cell">{entry.isDir ? "--" : formatBytes(entry.size)}</span>
                  <span className="sftp-cell">{entry.permissions ?? "----------"}</span>
                </button>
              )
            })}

          {!isLoading && !error && listing?.entries.length === 0 && (
            <div className="text-muted-foreground flex min-h-[200px] flex-col items-center justify-center gap-3">
              <FolderPlus className="size-6" />
              <span className="text-sm">
                {t("sftp.emptyDescription", { defaultValue: "This folder is empty" })}
              </span>
            </div>
          )}
        </ScrollArea>
      </div>
    </div>
  )
}
