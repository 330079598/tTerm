import "@/components/SftpDrawer.css"
import React, { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { AlertCircle } from "lucide-react"
import { useTranslation } from "react-i18next"

import { sftpDialogReducer } from "@/components/SftpDrawer/sftpDialogReducer"
import { useSftpSelectionMode } from "@/components/SftpDrawer/useSftpSelectionMode"
import { useSftpEntryActions } from "@/components/SftpDrawer/useSftpEntryActions"
import { useSftpDeleteActions } from "@/components/SftpDrawer/useSftpDeleteActions"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { useConfig } from "@/contexts/ConfigContext"
import { useUserVerification } from "@/contexts/UserVerificationContext"
import { SftpDeleteTransferEvents } from "@/components/SftpDrawer/SftpDeleteTransferEvents"
import { SftpDrawerContent } from "@/components/SftpDrawer/SftpDrawerContent"
import { SftpDrawerHeader } from "@/components/SftpDrawer/SftpDrawerHeader"
import { SftpDialogs } from "@/components/SftpDrawer/SftpDialogs"
import { SftpEntryContextMenu } from "@/components/SftpDrawer/SftpEntryContextMenu"
import { preloadMaterialIcons } from "@/components/SftpDrawer/SftpEntryIcon"
import { SftpPermissionsDialog } from "@/components/SftpDrawer/SftpPermissionsDialog"
import { useSftpDragDrop } from "@/components/SftpDrawer/useSftpDragDrop"
import { useSftpPasteUpload } from "@/components/SftpDrawer/useSftpPasteUpload"
import { useSftpSelection } from "@/components/SftpDrawer/useSftpSelection"
import {
  createSftpSearchMatcher,
  DEFAULT_SFTP_SEARCH_OPTIONS,
  type SftpSearchOptions,
} from "@/components/SftpDrawer/sftpSearch"
import type {
  LoadSftpDirectory,
  SftpContextMenuState,
  SftpDirectoryEntry,
  SftpDirectoryListing,
  SftpDrawerProps,
} from "@/components/SftpDrawer/types"
import { useSftpTransfers } from "@/components/SftpDrawer/useSftpTransfers"
import { useSftpHistory, useSftpHistoryNavigation } from "@/components/SftpDrawer/useSftpHistory"
import {
  applySftpView,
  loadSftpViewPreferences,
  nextSftpSort,
  saveSftpViewPreferences,
  type SftpSortColumn,
  type SftpViewPreferences,
} from "@/components/SftpDrawer/sftpView"

const EMPTY_SFTP_ENTRIES: SftpDirectoryEntry[] = []

export const SftpDrawer: React.FC<SftpDrawerProps> = ({
  tabId,
  visible,
  isGlobalShortcutTarget,
  connection,
  onClose,
  onOpenRemoteFile,
}) => {
  const { t } = useTranslation()
  const { config } = useConfig()
  const { withVaultUnlock } = useUserVerification()
  const drawerRef = useRef<HTMLDivElement>(null)
  const [listing, setListing] = useState<SftpDirectoryListing | null>(null)
  const [activePath, setActivePath] = useState<string | null>(null)
  const [selectedPaths, setSelectedPaths] = useState<string[]>([])
  const [isSelectionMode, setIsSelectionMode] = useState(false)
  const [searchQuery, setSearchQuery] = useState("")
  const [searchOptions, setSearchOptions] = useState<SftpSearchOptions>(DEFAULT_SFTP_SEARCH_OPTIONS)
  const [isLoading, setIsLoading] = useState(false)
  const [isDeleting, setIsDeleting] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [contextMenu, setContextMenu] = useState<SftpContextMenuState | null>(null)
  const [dialog, dispatchDialog] = useReducer(sftpDialogReducer, { type: "none" })
  const [permissionEntries, setPermissionEntries] =
    useState<SftpDirectoryEntry[]>(EMPTY_SFTP_ENTRIES)
  const [viewPreferences, setViewPreferences] = useState(loadSftpViewPreferences)
  const { history, recordFailed, recordLoaded } = useSftpHistory()
  const listingCurrentPath = listing?.currentPath ?? null
  const createFolderName = dialog.type === "createFolder" ? dialog.folderName : ""
  const renameEntry = dialog.type === "rename" ? dialog.entry : null
  const renameNewName = dialog.type === "rename" ? dialog.newName : ""
  const deleteDialogEntries = dialog.type === "delete" ? dialog.entries : EMPTY_SFTP_ENTRIES
  const commandDeleteEntries = dialog.type === "commandDelete" ? dialog.entries : EMPTY_SFTP_ENTRIES
  const commandDeleteCommand = dialog.type === "commandDelete" ? dialog.command : ""

  // The drawer lives as long as its tab, so unmounting means the tab closed:
  // let the backend close the tab's pooled SFTP connections.
  useEffect(
    () => () => {
      void invoke("sftp_release_tab_connections", { tabId }).catch(console.error)
    },
    [tabId]
  )

  const loadDirectory = useCallback<LoadSftpDirectory>(
    async (path, options) => {
      if (!connection) {
        const message = t("sftp.errors.missingConnection", {
          defaultValue: "SSH connection is missing",
        })
        setError(message)
        if (options?.throwOnError) {
          throw new Error(message)
        }
        return
      }

      setIsLoading(true)
      setError(null)
      try {
        const nextListing = await withVaultUnlock(() =>
          invoke<SftpDirectoryListing>("sftp_list_directory", {
            tabId,
            connection,
            path: path ?? undefined,
          })
        )
        setListing(nextListing)
        recordLoaded(nextListing.currentPath, options?.historyIndex)
        setActivePath(null)
        setSelectedPaths([])
        setSearchQuery("")
      } catch (invokeError) {
        const message = String(invokeError)
        setError(message)
        recordFailed(options?.historyIndex)
        if (options?.throwOnError) {
          throw new Error(message, { cause: invokeError })
        }
      } finally {
        setIsLoading(false)
      }
    },
    [connection, recordFailed, recordLoaded, t, tabId, withVaultUnlock]
  )

  const {
    addTransfer,
    conflictDialog,
    downloadEntries,
    handleUploadDialog,
    handleUploadFolderDialog,
    transfersRef,
    updateTransfer,
    uploadPaths,
  } = useSftpTransfers({
    connection,
    listing,
    loadDirectory,
    setError,
    tabId,
  })

  const { handleDragEnter, handleDragLeave, handleDragOver, handleDrop, isDragActive } =
    useSftpDragDrop({
      isGlobalShortcutTarget,
      listing,
      loadDirectory,
      setError,
      uploadPaths,
      visible,
    })

  useSftpPasteUpload({
    enabled: config.sftp_paste_upload_enabled && isGlobalShortcutTarget,
    listing,
    setError,
    uploadPaths,
    visible,
  })

  const searchMatcher = useMemo(
    () => createSftpSearchMatcher(searchQuery, searchOptions),
    [searchOptions, searchQuery]
  )

  const updateViewPreferences = useCallback(
    (update: (current: SftpViewPreferences) => SftpViewPreferences) => {
      setViewPreferences((current) => {
        const next = update(current)
        saveSftpViewPreferences(next)
        return next
      })
    },
    []
  )

  const handleSortColumn = useCallback(
    (column: SftpSortColumn) => {
      updateViewPreferences((current) => ({
        ...current,
        sort: nextSftpSort(current.sort, column),
      }))
    },
    [updateViewPreferences]
  )

  const toggleShowHidden = useCallback(() => {
    updateViewPreferences((current) => ({ ...current, showHidden: !current.showHidden }))
  }, [updateViewPreferences])

  // The listing as shown: hidden entries dropped, rows in sort order. Range
  // selection, select-all and the table all follow this order.
  const viewListing = useMemo(
    () =>
      listing ? { ...listing, entries: applySftpView(listing.entries, viewPreferences) } : listing,
    [listing, viewPreferences]
  )

  const filteredListing = useMemo(() => {
    if (!viewListing || !searchMatcher.hasQuery) {
      return viewListing
    }

    return {
      ...viewListing,
      entries: viewListing.entries.filter(searchMatcher.matches),
    }
  }, [viewListing, searchMatcher])

  const toggleSearchOption = useCallback((option: keyof SftpSearchOptions) => {
    setSearchOptions((current) => ({
      ...current,
      [option]: !current[option],
    }))
  }, [])

  const { enterSelectionMode, exitSelectionMode, toggleSelectionMode } = useSftpSelectionMode({
    visible,
    isGlobalShortcutTarget,
    dialogType: dialog.type,
    filteredListing,
    isLoading,
    error,
    isSelectionMode,
    setIsSelectionMode,
    setSelectedPaths,
    setActivePath,
    contextMenu,
    setContextMenu,
    drawerRef,
  })

  const currentPath = listing?.currentPath ?? null
  const { canGoBack, canGoForward, canGoUp, goBack, goForward, goUp, handleSideButton } =
    useSftpHistoryNavigation({
      contextMenuOpen: contextMenu !== null,
      dialogOpen: dialog.type !== "none",
      drawerRef,
      history,
      isGlobalShortcutTarget,
      isLoading,
      loadDirectory,
      parentPath: listing?.parentPath ?? null,
      visible,
    })

  const runAndRefresh = useCallback(
    async (action: () => Promise<void>) => {
      setError(null)
      try {
        await action()
        await loadDirectory(currentPath)
      } catch (invokeError) {
        setError(String(invokeError))
      }
    },
    [currentPath, loadDirectory]
  )
  const {
    activeEntry,
    breadcrumbs,
    contextMenuEntry,
    handleActivateEntry,
    handleClearSelection,
    handleSelectRange,
    handleToggleEntrySelection,
    selectedEntries,
  } = useSftpSelection({
    activePath,
    contextMenu,
    listing,
    rangeEntries: filteredListing?.entries,
    selectedPaths,
    setActivePath,
    setSelectedPaths,
  })

  const {
    handleCreateDirectory,
    handleCreateDirectoryConfirm,
    handleRename,
    handleRenameConfirm,
    handleDownload,
    handleDownloadSelection,
    handleOpenEntry,
    handleEdit,
    handlePermissions,
    handleApplyPermissions,
    handleCopyPath,
  } = useSftpEntryActions({
    tabId,
    connection,
    t,
    dispatchDialog,
    createFolderName,
    listingCurrentPath,
    runAndRefresh,
    contextMenuEntry,
    activeEntry,
    listing,
    renameEntry,
    renameNewName,
    selectedEntries,
    selectedPaths,
    downloadEntries,
    loadDirectory,
    onOpenRemoteFile,
    setPermissionEntries,
    currentPath,
  })

  const {
    handleDeleteSelection,
    handleDelete,
    handleDeleteConfirm,
    handleCommandDeleteConfirm,
    handleSftpDeleteConfirm,
  } = useSftpDeleteActions({
    tabId,
    connection,
    t,
    isDeleting,
    setIsDeleting,
    selectedEntries,
    selectedPaths,
    contextMenuEntry,
    activeEntry,
    dispatchDialog,
    setError,
    setContextMenu,
    setListing,
    setSelectedPaths,
    setActivePath,
    deleteDialogEntries,
    commandDeleteEntries,
    commandDeleteCommand,
  })

  const contextSelectionCount = useMemo(() => {
    if (!contextMenuEntry) {
      return selectedPaths.length
    }

    return selectedPaths.includes(contextMenuEntry.path) ? selectedPaths.length : 1
  }, [contextMenuEntry, selectedPaths])

  useEffect(() => {
    const drawer = drawerRef.current
    if (!drawer) {
      return
    }

    if (visible) {
      drawer.removeAttribute("inert")
      // Usually done before the first directory listing arrives.
      preloadMaterialIcons().catch(console.error)
    } else {
      drawer.setAttribute("inert", "")
    }
  }, [visible])

  return (
    <div
      className={`sftp-drawer ${visible ? "is-open" : ""}`}
      aria-hidden={!visible}
      ref={drawerRef}
      onMouseDown={handleSideButton}
      onMouseUp={handleSideButton}
      style={
        {
          "--sftp-font-size": `calc(${Math.max(config.font_size - 2, 10)}px * var(--ui-font-scale, 1))`,
          "--sftp-font-size-small": `calc(${Math.max(config.font_size - 3, 9)}px * var(--ui-font-scale, 1))`,
          "--sftp-font-size-large": `calc(${Math.max(config.font_size, 12)}px * var(--ui-font-scale, 1))`,
          "--sftp-font-size-title": `calc(${Math.max(config.font_size + 4, 16)}px * var(--ui-font-scale, 1))`,
        } as React.CSSProperties
      }
    >
      <SftpDeleteTransferEvents
        addTransfer={addTransfer}
        listingCurrentPath={listing?.currentPath}
        loadDirectory={loadDirectory}
        setError={setError}
        setIsDeleting={setIsDeleting}
        t={t}
        tabId={tabId}
        transfersRef={transfersRef}
        updateTransfer={updateTransfer}
      />
      <SftpDrawerHeader
        breadcrumbs={breadcrumbs}
        isGlobalShortcutTarget={isGlobalShortcutTarget}
        canGoBack={canGoBack}
        canGoForward={canGoForward}
        canGoUp={canGoUp}
        goBack={goBack}
        goForward={goForward}
        goUp={goUp}
        clearSelection={handleClearSelection}
        handleCreateDirectory={handleCreateDirectory}
        handleDeleteSelection={handleDeleteSelection}
        handleDownloadSelection={handleDownloadSelection}
        handleUploadDialog={handleUploadDialog}
        handleUploadFolderDialog={handleUploadFolderDialog}
        isDeleting={isDeleting}
        isLoading={isLoading}
        listingCurrentPath={listing?.currentPath}
        loadDirectory={loadDirectory}
        onClose={onClose}
        visible={visible}
        searchError={searchMatcher.error?.message ?? null}
        searchOptions={searchOptions}
        searchQuery={searchQuery}
        isSelectionMode={isSelectionMode}
        selectedCount={selectedPaths.length}
        setSearchQuery={setSearchQuery}
        toggleSearchOption={toggleSearchOption}
        toggleSelectionMode={toggleSelectionMode}
        showHidden={viewPreferences.showHidden}
        toggleShowHidden={toggleShowHidden}
      />

      {error && (
        <Alert className="bg-destructive/10 text-destructive rounded-none border-x-0 border-t-0">
          <AlertCircle className="size-4" />
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}

      <SftpDrawerContent
        error={error}
        activePath={activePath}
        handleActivateEntry={handleActivateEntry}
        handleDragEnter={handleDragEnter}
        handleDragLeave={handleDragLeave}
        handleDragOver={handleDragOver}
        handleDrop={handleDrop}
        handleOpenEntry={handleOpenEntry}
        handleSelectRange={handleSelectRange}
        enterSelectionMode={enterSelectionMode}
        exitSelectionMode={exitSelectionMode}
        isSelectionMode={isSelectionMode}
        handleToggleEntrySelection={handleToggleEntrySelection}
        isDragActive={isDragActive}
        isLoading={isLoading}
        listing={viewListing}
        loadDirectory={loadDirectory}
        onSortColumn={handleSortColumn}
        sort={viewPreferences.sort}
        searchMatcher={searchMatcher}
        selectedPaths={selectedPaths}
        setContextMenu={setContextMenu}
      />

      <SftpEntryContextMenu
        contextMenu={contextMenu}
        contextMenuEntry={contextMenuEntry}
        handleCopyPath={handleCopyPath}
        handleDelete={handleDelete}
        handleDownload={handleDownload}
        handleEdit={handleEdit}
        handlePermissions={handlePermissions}
        handleRename={handleRename}
        isDeleting={isDeleting}
        onClose={() => setContextMenu(null)}
        selectionCount={contextSelectionCount}
      />

      <SftpDialogs
        dialog={dialog}
        dispatchDialog={dispatchDialog}
        handleCommandDeleteConfirm={handleCommandDeleteConfirm}
        handleCreateDirectoryConfirm={handleCreateDirectoryConfirm}
        handleDeleteConfirm={handleDeleteConfirm}
        handleRenameConfirm={handleRenameConfirm}
        handleSftpDeleteConfirm={handleSftpDeleteConfirm}
        isDeleting={isDeleting}
      />
      <SftpPermissionsDialog
        entries={permissionEntries}
        onApply={handleApplyPermissions}
        onClose={() => setPermissionEntries(EMPTY_SFTP_ENTRIES)}
      />
      {conflictDialog}
    </div>
  )
}
