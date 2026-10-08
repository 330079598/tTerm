import React, { useEffect, useMemo, useRef, useState } from "react"
import {
  AlertCircle,
  ArrowDown,
  ArrowUp,
  ArrowUpFromLine,
  File,
  FolderPlus,
  Loader2,
  RefreshCcw,
} from "lucide-react"
import type { TFunction } from "i18next"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { ScrollArea } from "@/components/ui/scroll-area"
import { isMacPlatform } from "@/lib/keymap/chord"
import { cn } from "@/lib/utils"

import { SftpEntryIcon } from "@/components/SftpDrawer/SftpEntryIcon"
import { formatBytes, formatTimestamp } from "@/components/SftpDrawer/sftpDrawerUtils"
import type { SftpSearchMatcher } from "@/components/SftpDrawer/sftpSearch"
import {
  SFTP_SORT_COLUMNS,
  type SftpSort,
  type SftpSortColumn,
} from "@/components/SftpDrawer/sftpView"
import type {
  LoadSftpDirectory,
  SftpContextMenuState,
  SftpDirectoryEntry,
  SftpDirectoryListing,
} from "@/components/SftpDrawer/types"

const SFTP_COLUMN_WIDTH_STORAGE_KEY = "tterm.sftp.columnWidths"
const SFTP_COLUMN_DEFAULT_WEIGHTS = [300, 170, 100, 100, 126, 148] as const
const SFTP_COLUMN_MIN_WEIGHTS = [180, 130, 78, 70, 92, 110] as const

type ResizeState = {
  index: number
  startX: number
  pxPerWeight: number
  startWeights: number[]
}

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

function normalizeColumnWeight(weight: number) {
  return Math.round(weight * 10) / 10
}

function resizeColumnPair(startWeights: number[], index: number, deltaWeight: number) {
  const nextWeights = [...startWeights]
  const leftMin = SFTP_COLUMN_MIN_WEIGHTS[index]
  const rightMin = SFTP_COLUMN_MIN_WEIGHTS[index + 1]
  const pairTotal = startWeights[index] + startWeights[index + 1]
  const minLeft = Math.min(leftMin, pairTotal - rightMin)
  const maxLeft = Math.max(minLeft, pairTotal - rightMin)
  const requestedLeft = startWeights[index] + deltaWeight
  const nextLeft = Math.min(Math.max(requestedLeft, minLeft), maxLeft)

  nextWeights[index] = normalizeColumnWeight(nextLeft)
  nextWeights[index + 1] = normalizeColumnWeight(pairTotal - nextLeft)
  return nextWeights
}

function getInitialColumnWeights() {
  try {
    const storedWidths = window.localStorage.getItem(SFTP_COLUMN_WIDTH_STORAGE_KEY)
    if (!storedWidths) {
      return [...SFTP_COLUMN_DEFAULT_WEIGHTS]
    }

    const parsedWidths = JSON.parse(storedWidths)
    if (
      !Array.isArray(parsedWidths) ||
      parsedWidths.length !== SFTP_COLUMN_DEFAULT_WEIGHTS.length
    ) {
      return [...SFTP_COLUMN_DEFAULT_WEIGHTS]
    }

    return SFTP_COLUMN_DEFAULT_WEIGHTS.map((defaultWidth, index) => {
      const width = parsedWidths[index]
      return typeof width === "number" && Number.isFinite(width)
        ? Math.max(SFTP_COLUMN_MIN_WEIGHTS[index], normalizeColumnWeight(width))
        : defaultWidth
    })
  } catch {
    return [...SFTP_COLUMN_DEFAULT_WEIGHTS]
  }
}

interface SftpDrawerContentProps {
  activePath: string | null
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
  selectedPaths: string[]
  setContextMenu: React.Dispatch<React.SetStateAction<SftpContextMenuState | null>>
  sort: SftpSort
}

export const SftpDrawerContent: React.FC<SftpDrawerContentProps> = ({
  activePath,
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
  selectedPaths,
  setContextMenu,
  sort,
}) => {
  const { t } = useTranslation()
  const [isPointerSelecting, setIsPointerSelecting] = useState(false)
  const [columnWeights, setColumnWeights] = useState(getInitialColumnWeights)
  const pointerAnchorRef = useRef<string | null>(null)
  const pointerMovedRef = useRef(false)
  // Set when a press starts on the empty space below the rows, so a plain
  // click there (no rows swept) can clear the selection on release.
  const pointerFromBlankRef = useRef(false)
  const resizeStateRef = useRef<ResizeState | null>(null)
  const rowRefs = useRef(new Map<string, HTMLDivElement>())
  const tableShellRef = useRef<HTMLDivElement>(null)
  const gridTemplateColumns = useMemo(
    () => columnWeights.map((width) => `minmax(0, ${width}fr)`).join(" "),
    [columnWeights]
  )
  const compactGridTemplateColumns = useMemo(
    () =>
      columnWeights
        .slice(0, 3)
        .map((width) => `minmax(0, ${width}fr)`)
        .join(" "),
    [columnWeights]
  )
  const minimalGridTemplateColumns = useMemo(
    () =>
      columnWeights
        .slice(0, 2)
        .map((width) => `minmax(0, ${width}fr)`)
        .join(" "),
    [columnWeights]
  )
  const singleGridTemplateColumns = useMemo(
    () => `minmax(0, ${columnWeights[0]}fr)`,
    [columnWeights]
  )
  const tableGridStyle = useMemo(
    () =>
      ({
        "--sftp-grid-template": gridTemplateColumns,
        "--sftp-grid-template-compact": compactGridTemplateColumns,
        "--sftp-grid-template-minimal": minimalGridTemplateColumns,
        "--sftp-grid-template-single": singleGridTemplateColumns,
      }) as React.CSSProperties,
    [
      compactGridTemplateColumns,
      gridTemplateColumns,
      minimalGridTemplateColumns,
      singleGridTemplateColumns,
    ]
  )

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

  const updateSelectionFromPointer = React.useCallback(
    (clientY: number) => {
      const hoveredEntry = filteredEntries.find((entry) => {
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

      // Sweeping a row starts a multi-selection from any mode, so no toolbar
      // toggle is needed first. A press that stays on one row is still a click.
      if (!pointerAnchorRef.current) {
        pointerAnchorRef.current = hoveredEntry.path
        pointerMovedRef.current = true
        enterSelectionMode()
        handleActivateEntry(hoveredEntry.path)
        handleSelectRange(hoveredEntry.path, hoveredEntry.path)
        return
      }

      if (pointerAnchorRef.current === hoveredEntry.path) {
        return
      }

      pointerMovedRef.current = true
      enterSelectionMode()
      handleSelectRange(pointerAnchorRef.current, hoveredEntry.path)
    },
    [enterSelectionMode, filteredEntries, handleActivateEntry, handleSelectRange]
  )

  const startColumnResize = React.useCallback(
    (index: number, event: React.MouseEvent<HTMLSpanElement>) => {
      event.preventDefault()
      event.stopPropagation()
      const totalWeight = columnWeights.reduce((sum, width) => sum + width, 0)
      const availableWidth = tableShellRef.current?.clientWidth ?? totalWeight
      resizeStateRef.current = {
        index,
        startX: event.clientX,
        pxPerWeight: Math.max(availableWidth / totalWeight, 0.1),
        startWeights: columnWeights,
      }
      document.body.classList.add("sftp-column-resizing")
    },
    [columnWeights]
  )

  const adjustColumnWidth = React.useCallback((index: number, delta: number) => {
    setColumnWeights((currentWeights) => resizeColumnPair(currentWeights, index, delta))
  }, [])

  useEffect(() => {
    window.localStorage.setItem(SFTP_COLUMN_WIDTH_STORAGE_KEY, JSON.stringify(columnWeights))
  }, [columnWeights])

  useEffect(() => {
    const handleMouseMove = (event: MouseEvent) => {
      const resizeState = resizeStateRef.current
      if (!resizeState) {
        return
      }

      const delta = event.clientX - resizeState.startX
      setColumnWeights(
        resizeColumnPair(
          resizeState.startWeights,
          resizeState.index,
          delta / resizeState.pxPerWeight
        )
      )
    }

    const stopResize = () => {
      resizeStateRef.current = null
      document.body.classList.remove("sftp-column-resizing")
    }

    window.addEventListener("mousemove", handleMouseMove)
    window.addEventListener("mouseup", stopResize)
    return () => {
      document.body.classList.remove("sftp-column-resizing")
      window.removeEventListener("mousemove", handleMouseMove)
      window.removeEventListener("mouseup", stopResize)
    }
  }, [])

  useEffect(() => {
    if (!isPointerSelecting) {
      return
    }

    const handleMouseMove = (event: MouseEvent) => {
      event.preventDefault()
      updateSelectionFromPointer(event.clientY)
    }

    const stopSelection = () => {
      if (pointerFromBlankRef.current && !pointerMovedRef.current) {
        exitSelectionMode()
      }

      setIsPointerSelecting(false)
      pointerAnchorRef.current = null
      pointerFromBlankRef.current = false
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
  }, [exitSelectionMode, isPointerSelecting, updateSelectionFromPointer])

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
          <div
            className={cn("sftp-table-header", isSelectionMode && "sftp-table-header-selection")}
            role="row"
            style={tableGridStyle}
          >
            {isSelectionMode && <span className="sftp-header-cell" aria-hidden="true" />}
            {[
              t("sftp.columns.name"),
              t("sftp.columns.modified"),
              t("sftp.columns.size"),
              t("sftp.columns.kind", { defaultValue: "Kind" }),
              t("sftp.columns.permissions", { defaultValue: "Permissions" }),
              resultSummary ?? t("sftp.columns.owner"),
            ].map((label, index) => {
              const column = SFTP_SORT_COLUMNS[index]
              const isLast = index === SFTP_COLUMN_DEFAULT_WEIGHTS.length - 1
              // The last cell doubles as the result summary, which is not a column title.
              const sortable = !(isLast && resultSummary)
              const sorted = sortable && sort.column === column
              return (
                <span
                  key={index}
                  className={cn("sftp-header-cell", isLast && "sftp-header-summary")}
                  aria-live={isLast ? "polite" : undefined}
                  aria-sort={
                    sorted ? (sort.direction === "asc" ? "ascending" : "descending") : undefined
                  }
                >
                  {sortable ? (
                    <button
                      type="button"
                      className="sftp-header-sort"
                      onClick={() => onSortColumn(column)}
                      title={t("sftp.columns.sortBy", {
                        column: label,
                        defaultValue: `Sort by ${label}`,
                      })}
                    >
                      <span className="truncate">{label}</span>
                      {sorted &&
                        (sort.direction === "asc" ? (
                          <ArrowUp className="size-3 shrink-0" aria-hidden="true" />
                        ) : (
                          <ArrowDown className="size-3 shrink-0" aria-hidden="true" />
                        ))}
                    </button>
                  ) : (
                    label
                  )}
                  {index < SFTP_COLUMN_DEFAULT_WEIGHTS.length - 1 && (
                    <span
                      className="sftp-column-resizer"
                      role="separator"
                      tabIndex={0}
                      aria-orientation="vertical"
                      aria-valuenow={Math.round(columnWeights[index])}
                      aria-label={t("sftp.columns.resize", {
                        column: label,
                        defaultValue: `Resize ${label}`,
                      })}
                      onMouseDown={(event) => startColumnResize(index, event)}
                      onKeyDown={(event) => {
                        if (event.key === "ArrowLeft") {
                          event.preventDefault()
                          adjustColumnWidth(index, -12)
                        } else if (event.key === "ArrowRight") {
                          event.preventDefault()
                          adjustColumnWidth(index, 12)
                        }
                      }}
                    />
                  )}
                </span>
              )
            })}
          </div>
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

            pointerAnchorRef.current = null
            pointerFromBlankRef.current = true
            pointerMovedRef.current = false
            setIsPointerSelecting(true)
            // Cancelling the press (no text selection while dragging) also
            // cancels its focus change, which would leave keys going to the
            // covered terminal; take focus here as a plain click would.
            event.preventDefault()
            event.currentTarget.focus({ preventScroll: true })
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

                    pointerAnchorRef.current = entry.path
                    pointerMovedRef.current = false
                    pointerFromBlankRef.current = false
                    setIsPointerSelecting(true)
                    handleActivateEntry(entry.path)
                  }}
                  onClick={(event) => {
                    if (pointerMovedRef.current) {
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
                    if (pointerMovedRef.current) {
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
