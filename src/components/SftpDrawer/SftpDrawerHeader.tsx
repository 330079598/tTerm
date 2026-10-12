import React, { useEffect, useRef, useState } from "react"
import {
  ArrowLeft,
  ArrowRight,
  ArrowUp,
  ArrowDownToLine,
  ArrowUpFromLine,
  Eye,
  EyeOff,
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

import { SftpPathBar } from "@/components/SftpDrawer/SftpPathBar"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { cn } from "@/lib/utils"
import type { LoadSftpDirectory } from "@/components/SftpDrawer/types"
import type { SftpSearchOptions } from "@/components/SftpDrawer/sftpSearch"

export interface SftpDrawerHeaderProps {
  breadcrumbs: Array<{ label: string; path: string }>
  canGoBack: boolean
  canGoForward: boolean
  canGoUp: boolean
  clearSelection: () => void
  handleCreateDirectory: () => void
  handleDeleteSelection: () => void
  handleDownloadSelection: () => Promise<void>
  goBack: () => void
  goForward: () => void
  goUp: () => void
  handleUploadDialog: () => Promise<void>
  handleUploadFolderDialog: () => Promise<void>
  isDeleting: boolean
  isLoading: boolean
  isSelectionMode: boolean
  listingCurrentPath?: string | null
  loadDirectory: LoadSftpDirectory
  onClose: () => void
  /** The pane is the focused one: shortcuts are its own. */
  isGlobalShortcutTarget: boolean
  visible: boolean
  searchError: string | null
  searchOptions: SftpSearchOptions
  searchQuery: string
  selectedCount: number
  setSearchQuery: (query: string) => void
  toggleSearchOption: (option: keyof SftpSearchOptions) => void
  toggleSelectionMode: () => void
  showHidden: boolean
  toggleShowHidden: () => void
}

export const SftpDrawerHeader: React.FC<SftpDrawerHeaderProps> = ({
  breadcrumbs,
  canGoBack,
  canGoForward,
  canGoUp,
  clearSelection,
  handleCreateDirectory,
  handleDeleteSelection,
  handleDownloadSelection,
  goBack,
  goForward,
  goUp,
  handleUploadDialog,
  handleUploadFolderDialog,
  isDeleting,
  isLoading,
  isSelectionMode,
  listingCurrentPath,
  loadDirectory,
  onClose,
  isGlobalShortcutTarget,
  visible,
  searchError,
  searchOptions,
  searchQuery,
  selectedCount,
  setSearchQuery,
  toggleSearchOption,
  toggleSelectionMode,
  showHidden,
  toggleShowHidden,
}) => {
  const { t } = useTranslation()
  const [isSearchOpen, setIsSearchOpen] = useState(false)
  const searchInputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (isSearchOpen) {
      searchInputRef.current?.focus()
    }
  }, [isSearchOpen])

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

  const navigationButtons = [
    { key: "back", Icon: ArrowLeft, enabled: canGoBack, onClick: goBack },
    { key: "forward", Icon: ArrowRight, enabled: canGoForward, onClick: goForward },
    { key: "up", Icon: ArrowUp, enabled: canGoUp, onClick: goUp },
  ] as const
  const navigationLabels = {
    back: t("sftp.actions.back", { defaultValue: "Back" }),
    forward: t("sftp.actions.forward", { defaultValue: "Forward" }),
    up: t("sftp.actions.up", { defaultValue: "Parent Folder" }),
  }

  const selectionLabel = t("sftp.selection.selectedCount", {
    count: selectedCount,
    defaultValue: "{{count}} selected",
  })

  return (
    <div className="sftp-drawer-header">
      <div className="sftp-header-left">
        <span className="sftp-drawer-eyebrow">SFTP</span>
        <div className="sftp-nav-group">
          {navigationButtons.map(({ key, Icon, enabled, onClick }) => (
            <Tooltip key={key}>
              <TooltipTrigger asChild>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-xs"
                  onClick={onClick}
                  disabled={!enabled || isLoading}
                  aria-label={navigationLabels[key]}
                >
                  <Icon className="size-3.5" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>{navigationLabels[key]}</TooltipContent>
            </Tooltip>
          ))}
        </div>
        <SftpPathBar
          breadcrumbs={breadcrumbs}
          isGlobalShortcutTarget={isGlobalShortcutTarget}
          isLoading={isLoading}
          listingCurrentPath={listingCurrentPath}
          loadDirectory={loadDirectory}
          visible={visible}
        />
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
                  onClick={() => void handleDownloadSelection()}
                  aria-label={t("sftp.actions.downloadSelected", {
                    defaultValue: "Download Selection",
                  })}
                >
                  <ArrowDownToLine className="size-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                {t("sftp.actions.downloadSelected", { defaultValue: "Download Selection" })}
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
                onClick={toggleShowHidden}
                aria-pressed={showHidden}
                aria-label={t("sftp.actions.showHidden", { defaultValue: "Show hidden files" })}
              >
                {showHidden ? <Eye className="size-4" /> : <EyeOff className="size-4" />}
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              {showHidden
                ? t("sftp.actions.hideHidden", { defaultValue: "Hide hidden files" })
                : t("sftp.actions.showHidden", { defaultValue: "Show hidden files" })}
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
