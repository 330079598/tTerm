import React, { useEffect, useRef, useState } from "react"
import {
  ArrowUpFromLine,
  ChevronRight,
  FolderUp,
  FolderPlus,
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
import type { SftpSearchOptions } from "@/components/SftpDrawer/sftpSearch"

interface SftpDrawerHeaderProps {
  breadcrumbs: Array<{ label: string; path: string }>
  clearSelection: () => void
  handleCreateDirectory: () => void
  handleDeleteSelection: () => void
  handleUploadDialog: () => Promise<void>
  handleUploadFolderDialog: () => Promise<void>
  isLoading: boolean
  listingCurrentPath?: string | null
  loadDirectory: (path?: string | null) => Promise<void>
  onClose: () => void
  searchError: string | null
  searchOptions: SftpSearchOptions
  searchQuery: string
  selectedCount: number
  setSearchQuery: (query: string) => void
  toggleSearchOption: (option: keyof SftpSearchOptions) => void
}

export const SftpDrawerHeader: React.FC<SftpDrawerHeaderProps> = ({
  breadcrumbs,
  clearSelection,
  handleCreateDirectory,
  handleDeleteSelection,
  handleUploadDialog,
  handleUploadFolderDialog,
  isLoading,
  listingCurrentPath,
  loadDirectory,
  onClose,
  searchError,
  searchOptions,
  searchQuery,
  selectedCount,
  setSearchQuery,
  toggleSearchOption,
}) => {
  const { t } = useTranslation()
  const [isSearchOpen, setIsSearchOpen] = useState(false)
  const searchInputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (isSearchOpen) {
      searchInputRef.current?.focus()
    }
  }, [isSearchOpen])

  const handleToggleSearch = () => {
    setIsSearchOpen((current) => {
      if (current) {
        setSearchQuery("")
      }
      return !current
    })
  }

  return (
    <div className="sftp-drawer-header">
      <div className="sftp-header-left">
        <span className="sftp-drawer-eyebrow">SFTP</span>
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
      </div>
      <div className="sftp-header-actions">
        {isSearchOpen && (
          <div className="sftp-header-search">
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
                    {t("sftp.search.invalidRegex", { defaultValue: "Invalid regular expression" })}
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
        {selectedCount > 0 && (
          <>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button variant="ghost" size="icon-sm" onClick={clearSelection}>
                  <ListX className="size-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                {t("sftp.selection.clear", { defaultValue: "Clear selection" })}
              </TooltipContent>
            </Tooltip>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button variant="ghost" size="icon-sm" onClick={handleDeleteSelection}>
                  <Trash2 className="size-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                {t("sftp.actions.deleteSelected", { defaultValue: "Delete Selected" })}
              </TooltipContent>
            </Tooltip>
          </>
        )}
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon-sm"
              onClick={handleUploadDialog}
              disabled={!listingCurrentPath || isLoading}
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
            >
              <RefreshCcw className={cn("size-4", isLoading && "animate-spin")} />
            </Button>
          </TooltipTrigger>
          <TooltipContent>{t("sftp.actions.refresh", { defaultValue: "Refresh" })}</TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button variant="ghost" size="icon-sm" onClick={onClose}>
              <X className="size-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>{t("sftp.actions.close", { defaultValue: "Close" })}</TooltipContent>
        </Tooltip>
      </div>
    </div>
  )
}
