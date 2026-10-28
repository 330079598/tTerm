import "@/components/SftpDrawer.css"
import React, { useCallback, useReducer, useRef, useState } from "react"
import { AlertCircle } from "lucide-react"
import { useTranslation } from "react-i18next"

import { sftpDialogReducer } from "@/components/SftpDrawer/sftpDialogReducer"
import { sftpFontSizeStyle } from "@/components/SftpDrawer/sftpDrawerUtils"
import { useSftpSelectionMode } from "@/components/SftpDrawer/useSftpSelectionMode"
import { useSftpEntryActions } from "@/components/SftpDrawer/useSftpEntryActions"
import { useSftpDeleteActions } from "@/components/SftpDrawer/useSftpDeleteActions"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { useConfig } from "@/contexts/ConfigContext"
import { SftpDeleteTransferEvents } from "@/components/SftpDrawer/SftpDeleteTransferEvents"
import { SftpDrawerContent } from "@/components/SftpDrawer/SftpDrawerContent"
import { SftpDrawerHeader } from "@/components/SftpDrawer/SftpDrawerHeader"
import { SftpDialogs } from "@/components/SftpDrawer/SftpDialogs"
import { SftpEntryContextMenu } from "@/components/SftpDrawer/SftpEntryContextMenu"
import { SftpPermissionsDialog } from "@/components/SftpDrawer/SftpPermissionsDialog"
import { SftpArchiveDialog } from "@/components/SftpDrawer/SftpArchiveDialog"
import { SftpSearchStatus } from "@/components/SftpDrawer/SftpSearchStatus"
import { useSftpArchive } from "@/components/SftpDrawer/useSftpArchive"
import { useSftpRecursiveSearch } from "@/components/SftpDrawer/useSftpRecursiveSearch"
import { useSftpDragDrop } from "@/components/SftpDrawer/useSftpDragDrop"
import { useSftpDragOut } from "@/components/SftpDrawer/useSftpDragOut"
import { useSftpPasteUpload } from "@/components/SftpDrawer/useSftpPasteUpload"
import { useSftpSelection } from "@/components/SftpDrawer/useSftpSelection"
import { useSftpDirectoryLoader } from "@/components/SftpDrawer/useSftpDirectoryLoader"
import { useSftpTerminalFollow } from "@/components/SftpDrawer/useSftpTerminalFollow"
import { SftpTerminalActions } from "@/components/SftpDrawer/SftpTerminalActions"
import { useSftpDrawerLifecycle } from "@/components/SftpDrawer/useSftpDrawerLifecycle"
import { useSftpView } from "@/components/SftpDrawer/useSftpView"
import type {
  SftpContextMenuState,
  SftpDirectoryEntry,
  SftpDrawerProps,
} from "@/components/SftpDrawer/types"
import { useSftpTransfers } from "@/components/SftpDrawer/useSftpTransfers"
import { useSftpHistory, useSftpHistoryNavigation } from "@/components/SftpDrawer/useSftpHistory"

const EMPTY_SFTP_ENTRIES: SftpDirectoryEntry[] = []

export const SftpDrawer: React.FC<SftpDrawerProps> = ({
  tabId,
  visible,
  isGlobalShortcutTarget,
  connection,
  onClose,
  onOpenRemoteFile,
  onCdInTerminal,
}) => {
  const { t } = useTranslation()
  const { config } = useConfig()
  const drawerRef = useRef<HTMLDivElement>(null)
  const [activePath, setActivePath] = useState<string | null>(null)
  const [selectedPaths, setSelectedPaths] = useState<string[]>([])
  const [isSelectionMode, setIsSelectionMode] = useState(false)
  const [isDeleting, setIsDeleting] = useState(false)
  const [contextMenu, setContextMenu] = useState<SftpContextMenuState | null>(null)
  const [dialog, dispatchDialog] = useReducer(sftpDialogReducer, { type: "none" })
  const [permissionEntries, setPermissionEntries] =
    useState<SftpDirectoryEntry[]>(EMPTY_SFTP_ENTRIES)
  const { history, recordFailed, recordLoaded } = useSftpHistory()
  const [searchQuery, setSearchQuery] = useState("")

  const { listing, setListing, isLoading, error, setError, loadDirectory } = useSftpDirectoryLoader(
    {
      tabId,
      connection,
      recordLoaded,
      recordFailed,
      setActivePath,
      setSelectedPaths,
      setSearchQuery,
    }
  )
  const listingCurrentPath = listing?.currentPath ?? null
  const recursiveSearch = useSftpRecursiveSearch({ tabId, connection, listing, searchQuery })
  const { follows, toggleFollow, terminalCwd, awayHost } = useSftpTerminalFollow({
    tabId,
    connection,
    visible,
    listing,
    isLoading,
    loadDirectory,
  })
  const {
    searchOptions,
    toggleSearchOption,
    searchMatcher,
    listingMatcher,
    viewPreferences,
    handleSortColumn,
    toggleShowHidden,
    viewListing,
    filteredListing,
  } = useSftpView(recursiveSearch.shownListing, searchQuery)

  const {
    addTransfer,
    conflictDialog,
    downloadEntries,
    downloadEntriesToFolder,
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
  const archive = useSftpArchive({
    tabId,
    connection,
    addTransfer,
    updateTransfer,
    listingCurrentPath,
    loadDirectory,
  })

  const { handleDragEnter, handleDragLeave, handleDragOver, handleDrop, isDragActive } =
    useSftpDragDrop({
      drawerRef,
      isGlobalShortcutTarget,
      listing,
      setError,
      uploadPaths,
      visible,
    })

  const dragOut = useSftpDragOut({ downloadEntriesToFolder, setError })

  useSftpPasteUpload({
    enabled: config.sftp_paste_upload_enabled && isGlobalShortcutTarget,
    listing,
    setError,
    uploadPaths,
    visible,
  })

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

  const {
    activeEntry,
    breadcrumbs,
    contextMenuEntry,
    contextSelectionCount,
    handleActivateEntry,
    handleClearSelection,
    handleSelectRange,
    handleToggleEntrySelection,
    selectedEntries,
  } = useSftpSelection({
    activePath,
    contextMenu,
    listing: recursiveSearch.shownListing,
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
    handleCompress,
    handleExtract,
    handleReveal,
    handleCopyPath,
  } = useSftpEntryActions({
    tabId,
    connection,
    t,
    dispatchDialog,
    dialog,
    listingCurrentPath,
    setError,
    contextMenuEntry,
    activeEntry,
    listing,
    selectedEntries,
    selectedPaths,
    downloadEntries,
    loadDirectory,
    onOpenRemoteFile,
    setPermissionEntries,
    currentPath,
    setActivePath,
    openCompress: archive.openCompress,
    openExtract: archive.openExtract,
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
    dialog,
  })

  const handleCdInTerminal = useCallback(
    (path: string) => {
      const refusal = onCdInTerminal?.(path)
      if (refusal?.reason === "busy") setError(t("sftp.errors.terminalBusy"))
      if (refusal?.reason === "elsewhere") {
        setError(t("sftp.errors.terminalElsewhere", { host: refusal.host }))
      }
    },
    [onCdInTerminal, setError, t]
  )

  useSftpDrawerLifecycle(tabId, visible, drawerRef)

  return (
    <div
      className={`sftp-drawer ${visible ? "is-open" : ""}`}
      aria-hidden={!visible}
      ref={drawerRef}
      onMouseDown={handleSideButton}
      onMouseUp={handleSideButton}
      style={sftpFontSizeStyle(config.font_size)}
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
        onSubmitSearch={() =>
          void recursiveSearch.start(searchOptions.regex, viewPreferences.showHidden)
        }
        toggleSelectionMode={toggleSelectionMode}
        showHidden={viewPreferences.showHidden}
        toggleShowHidden={toggleShowHidden}
        terminalActions={
          <SftpTerminalActions
            follows={follows}
            toggleFollow={toggleFollow}
            terminalCwd={terminalCwd}
            awayHost={awayHost}
            shellIntegration={connection?.shellIntegration === true}
            currentPath={onCdInTerminal ? listingCurrentPath : null}
            cdInTerminal={handleCdInTerminal}
          />
        }
      />

      {error && (
        <Alert className="bg-destructive/10 text-destructive rounded-none border-x-0 border-t-0">
          <AlertCircle className="size-4" />
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}

      <SftpSearchStatus
        search={recursiveSearch.search}
        onStop={recursiveSearch.stop}
        onClose={() => setSearchQuery("")}
      />

      <SftpDrawerContent
        error={error}
        activePath={activePath}
        dragOut={dragOut}
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
        searchMatcher={listingMatcher}
        searchStatus={recursiveSearch.search?.status ?? null}
        selectedEntries={selectedEntries}
        selectedPaths={selectedPaths}
        setContextMenu={setContextMenu}
      />

      <SftpEntryContextMenu
        contextMenu={contextMenu}
        contextMenuEntry={contextMenuEntry}
        handleCdInTerminal={onCdInTerminal ? handleCdInTerminal : undefined}
        handleCompress={handleCompress}
        handleCopyPath={handleCopyPath}
        handleDelete={handleDelete}
        handleDownload={handleDownload}
        handleEdit={handleEdit}
        handleExtract={handleExtract}
        handlePermissions={handlePermissions}
        handleRename={handleRename}
        handleReveal={recursiveSearch.search ? handleReveal : undefined}
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
      <SftpArchiveDialog {...archive.dialogProps} />
      {conflictDialog}
    </div>
  )
}
