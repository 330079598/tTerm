import React, { useMemo } from "react"
import { AlertCircle, ArrowUpFromLine, File, FolderPlus, RefreshCcw } from "lucide-react"
import type { TFunction } from "i18next"
import { useTranslation } from "react-i18next"

import { useSftpColumnWidths } from "@/components/SftpDrawer/useSftpColumnWidths"
import type { SftpDragOut } from "@/components/SftpDrawer/useSftpDragOut"
import { useSftpPointerSelection } from "@/components/SftpDrawer/useSftpPointerSelection"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Spinner } from "@/components/ui/spinner"
import { isMacPlatform } from "@/lib/keymap/chord"
import { cn } from "@/lib/utils"
import { SftpTableHeader } from "@/components/SftpDrawer/SftpTableHeader"
import { SftpEntryIcon } from "@/components/SftpDrawer/SftpEntryIcon"
import { formatBytes, formatTimestamp } from "@/components/SftpDrawer/sftpDrawerUtils"
import type { SftpSearchMatcher } from "@/components/SftpDrawer/sftpSearch"
import type { SftpSort, SftpSortColumn } from "@/components/SftpDrawer/sftpView"
import type {
  LoadSftpDirectory,
  SftpContextMenuState,
  SftpDirectoryEntry,
  SftpDirectoryListing,
} from "@/components/SftpDrawer/types"

function getEntryKindLabel(entry: SftpDirectoryEntry, t: TFunction) {
  if (entry.isSymlink) {
    return t("sftp.kinds.symlink", { defaultValue: "Symlink" })
  }

  if (entry.isDir) {
    return t("sftp.kinds.folder", { defaultValue: "Folder" })
  }

  return t("sftp.kinds.file", { defaultValue: "File" })
}

function getOwnerGroupLabel(entry: SftpDirectoryEntry) {
  const owner = entry.owner?.trim()
  const group = entry.group?.trim()

  if (owner && group) {
    return `${owner}/${group}`
  }

  return owner || group || "--"
}

export interface SftpDrawerContentProps {
  activePath: string | null
  /** Null where dragging rows out to the file manager is unsupported. */
  dragOut: SftpDragOut | null
  enterSelectionMode: () => void
  error: string | null
  exitSelectionMode: () => void
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
  isSelectionMode: boolean
  listing: SftpDirectoryListing | null
  loadDirectory: LoadSftpDirectory
  onSortColumn: (column: SftpSortColumn) => void
  searchMatcher: SftpSearchMatcher
  selectedEntries: SftpDirectoryEntry[]
  selectedPaths: string[]
  setContextMenu: React.Dispatch<React.SetStateAction<SftpContextMenuState | null>>
  sort: SftpSort
}

export const SftpDrawerContent: React.FC<SftpDrawerContentProps> = ({
  activePath,
  dragOut,
  enterSelectionMode,
  error,
  exitSelectionMode,
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
  isSelectionMode,
  listing,
  loadDirectory,
  onSortColumn,
  searchMatcher,
  selectedEntries,
  selectedPaths,
  setContextMenu,
  sort,
}) => {
  const { t } = useTranslation()
  const { columnWeights, tableShellRef, tableGridStyle, startColumnResize, adjustColumnWidth } =
    useSftpColumnWidths()
  const filteredEntries = useMemo(() => {
    if (!searchMatcher.hasQuery) {
      return listing?.entries ?? []
    }

    return (listing?.entries ?? []).filter(searchMatcher.matches)
  }, [listing?.entries, searchMatcher])
  const showTableHeader = !isLoading && !error && listing && listing.entries.length > 0
  const resultSummary =
    !isLoading && !error && listing && (searchMatcher.hasQuery || isSelectionMode)
      ? searchMatcher.hasQuery
        ? t("sftp.search.resultSummary", {
            count: filteredEntries.length,
            total: listing.entries.length,
            defaultValue: "{{count}} of {{total}} items",
          })
        : t("sftp.itemSummary", {
            count: listing.entries.length,
            defaultValue: "{{count}} items",
          })
      : null

  const { pointerMovedRef, rowRefs, startBlankSweep, startRowSweep } = useSftpPointerSelection({
    entries: filteredEntries,
    enterSelectionMode,
    exitSelectionMode,
    handleActivateEntry,
    handleSelectRange,
  })

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

      <div className="sftp-table-shell" ref={tableShellRef}>
        {showTableHeader && (
          <SftpTableHeader
            isSelectionMode={isSelectionMode}
            tableGridStyle={tableGridStyle}
            resultSummary={resultSummary}
            sort={sort}
            onSortColumn={onSortColumn}
            columnWeights={columnWeights}
            startColumnResize={startColumnResize}
            adjustColumnWidth={adjustColumnWidth}
          />
        )}
        <ScrollArea
          className="flex-1 outline-none"
          tabIndex={-1}
          onMouseDown={(event) => {
            if (event.button !== 0) {
              return
            }

            const target = event.target as HTMLElement
            if (target.closest(".sftp-row") || target.closest("button")) {
              return
            }

            // A press on the native scrollbar lands on the scroll container
            // itself, past its client width; dragging it must not select rows.
            if (
              target === event.currentTarget &&
              event.nativeEvent.offsetX >= event.currentTarget.clientWidth
            ) {
              return
            }

            startBlankSweep()
            // Cancelling the press (no text selection while dragging) also
            // cancels its focus change, which would leave keys going to the
            // covered terminal; take focus here as a plain click would.
            event.preventDefault()
            event.currentTarget.focus({ preventScroll: true })
          }}
        >
          {isLoading && (
            <div className="text-muted-foreground flex min-h-[200px] flex-col items-center justify-center gap-3">
              <Spinner size={24} />
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
            filteredEntries.map((entry) => {
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
                    isSelectionMode && "sftp-row-selection-mode",
                    isActive && "sftp-row-active",
                    isSelected && "sftp-row-selected"
                  )}
                  style={tableGridStyle}
                  role="button"
                  tabIndex={0}
                  aria-pressed={isSelectionMode ? isSelected : isActive}
                  onMouseDown={(event) => {
                    // Modifier clicks are handled in onClick; on macOS a
                    // Control-click is a right click and opens the menu.
                    if (event.button !== 0 || event.shiftKey || event.metaKey || event.ctrlKey) {
                      return
                    }

                    const startedFromCheckbox = (event.target as HTMLElement).closest(
                      "[role='checkbox']"
                    )
                    if (startedFromCheckbox) {
                      return
                    }

                    // Pressing a highlighted row drags it (or the whole
                    // selection) out to the file manager; any other row
                    // starts sweeping a selection.
                    const dragEntries = isSelectionMode
                      ? isSelected && selectedEntries
                      : isActive && [entry]
                    if (dragOut && dragEntries) {
                      dragOut.beginPress(event, dragEntries)
                    } else {
                      startRowSweep(entry.path)
                    }
                    handleActivateEntry(entry.path)
                  }}
                  onClick={(event) => {
                    if (pointerMovedRef.current || dragOut?.startedRef.current) {
                      event.preventDefault()
                      event.stopPropagation()
                      return
                    }

                    // Shift extends from the last clicked row, which stays the
                    // anchor; Cmd (Ctrl elsewhere) toggles one row. Either one
                    // enters selection mode, keeping the row highlighted
                    // before it as part of the selection.
                    if (event.shiftKey) {
                      enterSelectionMode()
                      handleSelectRange(activePath ?? entry.path, entry.path)
                      setContextMenu(null)
                      return
                    }

                    if (isMacPlatform() ? event.metaKey : event.ctrlKey) {
                      if (
                        !isSelectionMode &&
                        activePath &&
                        activePath !== entry.path &&
                        !selectedPaths.includes(activePath)
                      ) {
                        handleToggleEntrySelection(activePath, true)
                      }
                      enterSelectionMode()
                      handleToggleEntrySelection(entry.path, !isSelected)
                      handleActivateEntry(entry.path)
                      setContextMenu(null)
                      return
                    }

                    if (isSelectionMode) {
                      handleToggleEntrySelection(entry.path, !isSelected)
                      handleActivateEntry(entry.path)
                      setContextMenu(null)
                      return
                    }

                    handleActivateEntry(entry.path)
                    setContextMenu(null)
                  }}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") {
                      event.preventDefault()
                      if (isSelectionMode) {
                        handleToggleEntrySelection(entry.path, !isSelected)
                        handleActivateEntry(entry.path)
                        setContextMenu(null)
                        return
                      }

                      void handleOpenEntry(entry)
                      return
                    }

                    if (event.key === " " && isSelectionMode) {
                      event.preventDefault()
                      handleToggleEntrySelection(entry.path, !isSelected)
                      handleActivateEntry(entry.path)
                      setContextMenu(null)
                    }
                  }}
                  onDoubleClick={() => {
                    if (pointerMovedRef.current || dragOut?.startedRef.current) {
                      return
                    }

                    if (!isSelectionMode) {
                      void handleOpenEntry(entry)
                    }
                  }}
                  onContextMenu={(event) => {
                    event.preventDefault()
                    handleActivateEntry(entry.path)
                    setContextMenu({ x: event.clientX, y: event.clientY, entryPath: entry.path })
                  }}
                  data-allow-context-menu
                >
                  {isSelectionMode && (
                    <span className="sftp-cell sftp-checkbox-cell">
                      <Checkbox
                        checked={isSelected}
                        onClick={(event) => {
                          event.stopPropagation()
                        }}
                        onCheckedChange={(checked) => {
                          handleToggleEntrySelection(entry.path, checked)
                          handleActivateEntry(entry.path)
                          setContextMenu(null)
                        }}
                        aria-label={t("sftp.selection.toggle", {
                          name: entry.name,
                          defaultValue: `Select ${entry.name}`,
                        })}
                      />
                    </span>
                  )}
                  <span className="sftp-cell sftp-name-cell">
                    <SftpEntryIcon entry={entry} />
                    <span className="truncate">{entry.name}</span>
                  </span>
                  <span className="sftp-cell">{formatTimestamp(entry.modifiedAt)}</span>
                  <span className="sftp-cell">{entry.isDir ? "--" : formatBytes(entry.size)}</span>
                  <span className="sftp-cell">{getEntryKindLabel(entry, t)}</span>
                  <span className="sftp-cell">{entry.permissions ?? "----------"}</span>
                  <span className="sftp-cell">{getOwnerGroupLabel(entry)}</span>
                </div>
              )
            })}

          {!isLoading &&
            !error &&
            listing &&
            listing.entries.length > 0 &&
            filteredEntries.length === 0 &&
            !searchMatcher.error && (
              <div className="sftp-empty-state">
                <File className="size-6" />
                <span className="sftp-empty-title">
                  {t("sftp.search.noResults", {
                    defaultValue: "No files or folders match this filter",
                  })}
                </span>
                <span className="sftp-empty-description">
                  {t("sftp.search.noResultsHint", {
                    defaultValue: "Try a broader text filter, glob, or regular expression.",
                  })}
                </span>
              </div>
            )}

          {!isLoading && !error && listing?.entries.length === 0 && !searchMatcher.error && (
            <div className="sftp-empty-state">
              <FolderPlus className="size-6" />
              <span className="sftp-empty-title">
                {t("sftp.emptyDescription", { defaultValue: "This folder is empty" })}
              </span>
              <span className="sftp-empty-description">
                {t("sftp.emptyHint", {
                  defaultValue: "Upload files or create a folder to start organizing this path.",
                })}
              </span>
            </div>
          )}
        </ScrollArea>
      </div>
    </div>
  )
}
