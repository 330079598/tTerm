import React, { useCallback, useEffect, useRef, useState } from "react"
import {
  ArrowUpFromLine,
  Check,
  ChevronRight,
  FolderInput,
  FolderUp,
  FolderPlus,
  ListChecks,
  ListX,
  RefreshCcw,
  Regex,
  Search,
  Trash2,
  X,
} from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { cn } from "@/lib/utils"
import type { LoadSftpDirectory } from "@/components/SftpDrawer/types"
import type { SftpSearchOptions } from "@/components/SftpDrawer/sftpSearch"

interface SftpDrawerHeaderProps {
  breadcrumbs: Array<{ label: string; path: string }>
  clearSelection: () => void
  handleCreateDirectory: () => void
  handleDeleteSelection: () => void
  handleUploadDialog: () => Promise<void>
  handleUploadFolderDialog: () => Promise<void>
  isDeleting: boolean
  isLoading: boolean
  isSelectionMode: boolean
  listingCurrentPath?: string | null
  loadDirectory: LoadSftpDirectory
  onClose: () => void
  visible: boolean
  searchError: string | null
  searchOptions: SftpSearchOptions
  searchQuery: string
  selectedCount: number
  setSearchQuery: (query: string) => void
  toggleSearchOption: (option: keyof SftpSearchOptions) => void
  toggleSelectionMode: () => void
}

export const SftpDrawerHeader: React.FC<SftpDrawerHeaderProps> = ({
  breadcrumbs,
  clearSelection,
  handleCreateDirectory,
  handleDeleteSelection,
  handleUploadDialog,
  handleUploadFolderDialog,
  isDeleting,
  isLoading,
  isSelectionMode,
  listingCurrentPath,
  loadDirectory,
  onClose,
  visible,
  searchError,
  searchOptions,
  searchQuery,
  selectedCount,
  setSearchQuery,
  toggleSearchOption,
  toggleSelectionMode,
}) => {
  const { t } = useTranslation()
  const [isPathEditing, setIsPathEditing] = useState(false)
  const [pathDraft, setPathDraft] = useState(listingCurrentPath ?? "")
  const [pathError, setPathError] = useState<string | null>(null)
  const [isSearchOpen, setIsSearchOpen] = useState(false)
  const pathInputRef = useRef<HTMLInputElement>(null)
  const searchInputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (isPathEditing) {
      pathInputRef.current?.focus()
      pathInputRef.current?.select()
    }
  }, [isPathEditing])

  useEffect(() => {
    if (isSearchOpen) {
      searchInputRef.current?.focus()
    }
  }, [isSearchOpen])

  const openPathEditor = useCallback(() => {
    if (!visible || !listingCurrentPath || isLoading) return

    setPathDraft(listingCurrentPath)
    setPathError(null)
    setIsPathEditing(true)
  }, [isLoading, listingCurrentPath, visible])

  const closePathEditor = () => {
    setIsPathEditing(false)
    setPathDraft(listingCurrentPath ?? "")
    setPathError(null)
  }

  const submitPath = async () => {
    const nextPath = pathDraft.trim()
    if (!nextPath || isLoading) return

    setPathError(null)
    try {
      await loadDirectory(nextPath, { throwOnError: true })
      setIsPathEditing(false)
    } catch (error) {
      setPathError(error instanceof Error ? error.message : String(error))
    }
  }

  const handlePathKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter") {
      event.preventDefault()
      void submitPath()
      return
    }

    if (event.key === "Escape") {
      event.preventDefault()
      closePathEditor()
    }
  }

  const handleCloseSearch = () => {
    setIsSearchOpen(false)
    setSearchQuery("")
  }

  const handleSearchKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== "Escape") return

    event.preventDefault()
    event.stopPropagation()
    handleCloseSearch()
  }

  const handleToggleSearch = () => {
    if (isSearchOpen) {
      handleCloseSearch()
      return
    }

    setIsSearchOpen(true)
  }

  useEffect(() => {
    const handleGlobalKeyDown = (event: KeyboardEvent) => {
      if (event.key.toLowerCase() !== "l" || (!event.ctrlKey && !event.metaKey) || event.altKey) {
        return
      }

      event.preventDefault()
      openPathEditor()
    }

    window.addEventListener("keydown", handleGlobalKeyDown)
    return () => window.removeEventListener("keydown", handleGlobalKeyDown)
  }, [openPathEditor])

  const selectionLabel = t("sftp.selection.selectedCount", {
    count: selectedCount,
    defaultValue: "{{count}} selected",
  })

  return (
    <div className="sftp-drawer-header">
      <div className="sftp-header-left">
        <span className="sftp-drawer-eyebrow">SFTP</span>
        {isPathEditing ? (
          <div className="sftp-path-editor">
            <FolderInput className="sftp-path-icon" />
            <Input
              ref={pathInputRef}
              value={pathDraft}
              onChange={(event) => setPathDraft(event.target.value)}
              onKeyDown={handlePathKeyDown}
              disabled={isLoading}
              className={cn("sftp-path-input", pathError && "border-destructive")}
              aria-describedby={pathError ? "sftp-path-error" : undefined}
              aria-invalid={Boolean(pathError)}
              aria-label={t("sftp.path.label", { defaultValue: "Go to remote path" })}
            />
            <div className="sftp-path-controls">
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-xs"
                    onClick={() => void submitPath()}
                    disabled={isLoading || !pathDraft.trim()}
                    aria-label={t("sftp.path.go", { defaultValue: "Go to path" })}
                  >
                    <Check className="size-3" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>{t("sftp.path.go", { defaultValue: "Go to path" })}</TooltipContent>
              </Tooltip>
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-xs"
                    onClick={closePathEditor}
                    aria-label={t("sftp.path.cancel", { defaultValue: "Cancel path edit" })}
                  >
                    <X className="size-3" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>
                  {t("sftp.path.cancel", { defaultValue: "Cancel path edit" })}
                </TooltipContent>
              </Tooltip>
            </div>
            {pathError && (
              <span id="sftp-path-error" className="sftp-path-error" role="alert">
                {pathError}
              </span>
            )}
          </div>
        ) : (
          <div className="sftp-path-display">
            <div className="sftp-breadcrumbs">
              {breadcrumbs.map((item, index) => (
                <React.Fragment key={item.path}>
                  {index > 0 && <ChevronRight className="text-muted-foreground size-3" />}
                  <Button
                    variant="ghost"
                    size="xs"
                    onClick={() => void loadDirectory(item.path)}
                    disabled={isLoading}
                    className="h-6 px-2"
                  >
                    {item.label}
                  </Button>
                </React.Fragment>
              ))}
            </div>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-xs"
                  onClick={openPathEditor}
                  disabled={!listingCurrentPath || isLoading}
                  aria-label={t("sftp.path.edit", { defaultValue: "Enter remote path" })}
                >
                  <FolderInput className="size-3.5" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                {t("sftp.path.edit", { defaultValue: "Enter remote path" })}
              </TooltipContent>
            </Tooltip>
          </div>
        )}
      </div>
      <div className="sftp-header-actions">
        <div className="sftp-action-group">
          {isSearchOpen && (
            <div className="sftp-header-search" onKeyDown={handleSearchKeyDown}>
              <div className="sftp-search-box">
                <Search className="sftp-search-icon" />
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Input
                      ref={searchInputRef}
                      value={searchQuery}
                      onChange={(event) => setSearchQuery(event.target.value)}
                      placeholder={t("sftp.search.placeholder", {
                        defaultValue: "Filter current folder",
                      })}
                      disabled={!listingCurrentPath || isLoading}
                      className={cn("sftp-search-input", searchError && "border-destructive")}
                      aria-invalid={Boolean(searchError)}
                      aria-label={t("sftp.search.label", { defaultValue: "Filter current folder" })}
                    />
                  </TooltipTrigger>
                  {searchError && <TooltipContent>{searchError}</TooltipContent>}
                </Tooltip>
                <div className="sftp-search-controls">
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button
                        type="button"
                        variant={searchOptions.regex ? "secondary" : "ghost"}
                        size="icon-xs"
                        onClick={() => toggleSearchOption("regex")}
                        aria-label={t("sftp.search.regex", {
                          defaultValue: "Use regular expression; falls back to glob",
                        })}
                        aria-pressed={searchOptions.regex}
                        disabled={!listingCurrentPath || isLoading}
                      >
                        <Regex className="size-3" />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>
                      {t("sftp.search.regex", {
                        defaultValue: "Use regular expression; falls back to glob",
                      })}
                    </TooltipContent>
                  </Tooltip>
                  {searchQuery && (
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <Button
                          variant="ghost"
                          size="icon-xs"
                          onClick={() => setSearchQuery("")}
                          aria-label={t("sftp.search.clear", { defaultValue: "Clear filter" })}
                        >
                          <X className="size-3" />
                        </Button>
                      </TooltipTrigger>
                      <TooltipContent>
                        {t("sftp.search.clear", { defaultValue: "Clear filter" })}
                      </TooltipContent>
                    </Tooltip>
                  )}
                </div>
              </div>
              {searchError && (
                <Tooltip>
                  <TooltipTrigger asChild>
                    <span className="sftp-search-error">
                      {t("sftp.search.invalidRegex", {
                        defaultValue: "Invalid regular expression",
                      })}
                    </span>
                  </TooltipTrigger>
                  <TooltipContent>{searchError}</TooltipContent>
                </Tooltip>
              )}
            </div>
          )}
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant={isSearchOpen || searchQuery ? "secondary" : "ghost"}
                size="icon-sm"
                onClick={handleToggleSearch}
                disabled={!listingCurrentPath || isLoading}
                aria-label={t("sftp.search.label", { defaultValue: "Filter current folder" })}
                aria-pressed={isSearchOpen || Boolean(searchQuery)}
              >
                <Search className="size-4" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              {t("sftp.search.label", { defaultValue: "Filter current folder" })}
            </TooltipContent>
          </Tooltip>
        </div>
        <div className="sftp-action-divider" aria-hidden="true" />
        <div className="sftp-action-group">
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant={isSelectionMode ? "secondary" : "ghost"}
                size="icon-sm"
                onClick={toggleSelectionMode}
                disabled={!listingCurrentPath || isLoading}
                aria-label={t("sftp.selection.mode", {
                  defaultValue: "Toggle selection mode",
                })}
                aria-pressed={isSelectionMode}
              >
                <ListChecks className="size-4" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              {t("sftp.selection.mode", { defaultValue: "Toggle selection mode" })}
            </TooltipContent>
          </Tooltip>
        </div>
        {selectedCount > 0 && (
          <div className="sftp-selection-actions" aria-live="polite">
            <span className="sftp-selection-count">{selectionLabel}</span>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  onClick={clearSelection}
                  aria-label={t("sftp.selection.clear", { defaultValue: "Clear selection" })}
                >
                  <ListX className="size-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                {t("sftp.selection.clear", { defaultValue: "Clear selection" })}
              </TooltipContent>
            </Tooltip>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  onClick={handleDeleteSelection}
                  disabled={isDeleting}
                  aria-label={t("sftp.actions.deleteSelected", { defaultValue: "Delete Selected" })}
                >
                  <Trash2 className="size-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                {t("sftp.actions.deleteSelected", { defaultValue: "Delete Selected" })}
              </TooltipContent>
            </Tooltip>
          </div>
        )}
        <div className="sftp-action-divider" aria-hidden="true" />
        <div className="sftp-action-group">
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon-sm"
                onClick={handleUploadDialog}
                disabled={!listingCurrentPath || isLoading}
                aria-label={t("sftp.actions.uploadFiles", { defaultValue: "Upload Files" })}
              >
                <ArrowUpFromLine className="size-4" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              {t("sftp.actions.uploadFiles", { defaultValue: "Upload Files" })}
            </TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon-sm"
                onClick={handleUploadFolderDialog}
                disabled={!listingCurrentPath || isLoading}
                aria-label={t("sftp.actions.uploadFolder", { defaultValue: "Upload Folder" })}
              >
                <FolderUp className="size-4" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              {t("sftp.actions.uploadFolder", { defaultValue: "Upload Folder" })}
            </TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon-sm"
                onClick={handleCreateDirectory}
                disabled={!listingCurrentPath || isLoading}
                aria-label={t("sftp.actions.newFolder", { defaultValue: "New Folder" })}
              >
                <FolderPlus className="size-4" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              {t("sftp.actions.newFolder", { defaultValue: "New Folder" })}
            </TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon-sm"
                onClick={() => void loadDirectory(listingCurrentPath ?? null)}
                disabled={isLoading}
                aria-label={t("sftp.actions.refresh", { defaultValue: "Refresh" })}
              >
                <RefreshCcw className={cn("size-4", isLoading && "animate-spin")} />
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              {t("sftp.actions.refresh", { defaultValue: "Refresh" })}
            </TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon-sm"
                onClick={onClose}
                aria-label={t("sftp.actions.close", { defaultValue: "Close" })}
              >
                <X className="size-4" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>{t("sftp.actions.close", { defaultValue: "Close" })}</TooltipContent>
          </Tooltip>
        </div>
      </div>
    </div>
  )
}
