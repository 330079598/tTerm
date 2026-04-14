import React, { useEffect, useRef, useState } from "react"
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
import { Checkbox } from "@/components/ui/checkbox"
import { ScrollArea } from "@/components/ui/scroll-area"
import { cn } from "@/lib/utils"

import { formatBytes, formatTimestamp } from "@/components/SftpDrawer/sftpDrawerUtils"
import type {
  SftpContextMenuState,
  SftpDirectoryEntry,
  SftpDirectoryListing,
} from "@/components/SftpDrawer/types"

interface SftpDrawerContentProps {
  activePath: string | null
  error: string | null
  handleActivateEntry: (path: string) => void
  handleDragEnter: (event: React.DragEvent<HTMLDivElement>) => void
  handleDragLeave: (event: React.DragEvent<HTMLDivElement>) => void
  handleDragOver: (event: React.DragEvent<HTMLDivElement>) => void
  handleDrop: (event: React.DragEvent<HTMLDivElement>) => Promise<void>
  handleOpenEntry: (entry: SftpDirectoryEntry) => void | Promise<void>
  handleSelectRange: (anchorPath: string, currentPath: string) => void
  handleToggleEntrySelection: (path: string, checked: boolean) => void
  isDragActive: boolean
  isLoading: boolean
  listing: SftpDirectoryListing | null
  loadDirectory: (path?: string | null) => Promise<void>
  selectedPaths: string[]
  setContextMenu: React.Dispatch<React.SetStateAction<SftpContextMenuState | null>>
}

export const SftpDrawerContent: React.FC<SftpDrawerContentProps> = ({
  activePath,
  error,
  handleActivateEntry,
  handleDragEnter,
  handleDragLeave,
  handleDragOver,
  handleDrop,
  handleOpenEntry,
  handleSelectRange,
  handleToggleEntrySelection,
  isDragActive,
  isLoading,
  listing,
  loadDirectory,
  selectedPaths,
  setContextMenu,
}) => {
  const { t } = useTranslation()
  const [isPointerSelecting, setIsPointerSelecting] = useState(false)
  const pointerAnchorRef = useRef<string | null>(null)
  const pointerMovedRef = useRef(false)
  const rowRefs = useRef(new Map<string, HTMLDivElement>())

  const updateSelectionFromPointer = React.useCallback(
    (clientY: number) => {
      if (!listing) {
        return
      }

      const hoveredEntry = listing.entries.find((entry) => {
        const row = rowRefs.current.get(entry.path)
        if (!row) {
          return false
        }

        const rect = row.getBoundingClientRect()
        return clientY >= rect.top && clientY <= rect.bottom
      })

      if (!hoveredEntry) {
        return
      }

      if (!pointerAnchorRef.current) {
        pointerAnchorRef.current = hoveredEntry.path
        handleActivateEntry(hoveredEntry.path)
        handleSelectRange(hoveredEntry.path, hoveredEntry.path)
        return
      }

      if (pointerAnchorRef.current !== hoveredEntry.path) {
        pointerMovedRef.current = true
      }

      handleSelectRange(pointerAnchorRef.current, hoveredEntry.path)
    },
    [handleActivateEntry, handleSelectRange, listing]
  )

  useEffect(() => {
    if (!isPointerSelecting) {
      return
    }

    const handleMouseMove = (event: MouseEvent) => {
      event.preventDefault()
      updateSelectionFromPointer(event.clientY)
    }

    const stopSelection = () => {
      setIsPointerSelecting(false)
      pointerAnchorRef.current = null
      document.body.classList.remove("sftp-no-select")
      window.setTimeout(() => {
        pointerMovedRef.current = false
      }, 0)
    }

    document.body.classList.add("sftp-no-select")
    window.addEventListener("mousemove", handleMouseMove)
    window.addEventListener("mouseup", stopSelection)
    return () => {
      document.body.classList.remove("sftp-no-select")
      window.removeEventListener("mousemove", handleMouseMove)
      window.removeEventListener("mouseup", stopSelection)
    }
  }, [isPointerSelecting, updateSelectionFromPointer])

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
        <ScrollArea
          className="flex-1"
          onMouseDown={(event) => {
            if (event.button !== 0) {
              return
            }

            const target = event.target as HTMLElement
            if (target.closest(".sftp-row") || target.closest("button")) {
              return
            }

            pointerAnchorRef.current = null
            pointerMovedRef.current = false
            setIsPointerSelecting(true)
            event.preventDefault()
          }}
        >
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
              const isSelected = selectedPaths.includes(entry.path)
              const isActive = entry.path === activePath
              return (
                <div
                  key={entry.path}
                  ref={(node) => {
                    if (node) {
                      rowRefs.current.set(entry.path, node)
                    } else {
                      rowRefs.current.delete(entry.path)
                    }
                  }}
                  className={cn(
                    "sftp-row",
                    isActive && "sftp-row-active",
                    isSelected && "sftp-row-selected"
                  )}
                  role="button"
                  tabIndex={0}
                  aria-pressed={isActive}
                  onMouseDown={(event) => {
                    if (event.button !== 0) {
                      return
                    }

                    const startedFromCheckbox = (event.target as HTMLElement).closest(
                      "[role='checkbox']"
                    )
                    if (startedFromCheckbox) {
                      return
                    }

                    pointerAnchorRef.current = entry.path
                    pointerMovedRef.current = false
                    setIsPointerSelecting(true)
                    handleActivateEntry(entry.path)
                    handleSelectRange(entry.path, entry.path)
                    event.preventDefault()
                  }}
                  onClick={(event) => {
                    if (pointerMovedRef.current) {
                      event.preventDefault()
                      event.stopPropagation()
                      return
                    }

                    handleActivateEntry(entry.path)
                    setContextMenu(null)
                  }}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") {
                      event.preventDefault()
                      void handleOpenEntry(entry)
                    }
                  }}
                  onDoubleClick={() => void handleOpenEntry(entry)}
                  onContextMenu={(event) => {
                    event.preventDefault()
                    handleActivateEntry(entry.path)
                    setContextMenu({ x: event.clientX, y: event.clientY, entryPath: entry.path })
                  }}
                >
                  <span className="sftp-cell sftp-checkbox-cell">
                    <Checkbox
                      checked={isSelected}
                      onClick={(event) => {
                        event.stopPropagation()
                      }}
                      onCheckedChange={(checked) => {
                        handleToggleEntrySelection(entry.path, checked)
                        setContextMenu(null)
                      }}
                      aria-label={t("sftp.selection.toggle", {
                        name: entry.name,
                        defaultValue: `Select ${entry.name}`,
                      })}
                    />
                  </span>
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
                </div>
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
