import React from "react"
import {
  ArrowLeft,
  ArrowRight,
  ArrowUp,
  ArrowDownToLine,
  Eye,
  EyeOff,
  FileUp,
  FolderUp,
  FolderPlus,
  ListChecks,
  ListX,
  RefreshCcw,
  Trash2,
  X,
} from "lucide-react"
import { useTranslation } from "react-i18next"

import { SftpHeaderSearch } from "@/components/SftpDrawer/SftpHeaderSearch"
import { SftpPathBar } from "@/components/SftpDrawer/SftpPathBar"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { useShortcutLabel } from "@/contexts/KeymapContext"
import { withShortcut } from "@/lib/keymap/keymap"
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
  /** Actions tied to the drawer's terminal, shown before the search. */
  terminalActions?: React.ReactNode
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
  terminalActions,
}) => {
  const { t } = useTranslation()
  const shortcutLabel = useShortcutLabel()
  const navigationButtons = [
    { key: "back", Icon: ArrowLeft, enabled: canGoBack, onClick: goBack, shortcut: "sftp.back" },
    {
      key: "forward",
      Icon: ArrowRight,
      enabled: canGoForward,
      onClick: goForward,
      shortcut: "sftp.forward",
    },
    { key: "up", Icon: ArrowUp, enabled: canGoUp, onClick: goUp, shortcut: "sftp.up" },
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
          {navigationButtons.map(({ key, Icon, enabled, onClick, shortcut }) => (
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
              <TooltipContent>
                {withShortcut(navigationLabels[key], shortcutLabel(shortcut))}
              </TooltipContent>
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
        {terminalActions}
        <div className="sftp-action-group">
          <SftpHeaderSearch
            disabled={!listingCurrentPath || isLoading}
            searchError={searchError}
            searchOptions={searchOptions}
            searchQuery={searchQuery}
            setSearchQuery={setSearchQuery}
            toggleSearchOption={toggleSearchOption}
          />
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
                <FileUp className="size-4" />
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
                {isLoading ? <Spinner /> : <RefreshCcw className="size-4" />}
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
