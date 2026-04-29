import React, { useEffect, useRef, useState } from "react"
import {
  ArrowUpFromLine,
  ChevronRight,
  FolderPlus,
  ListX,
  RefreshCcw,
  Search,
  Trash2,
  X,
} from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { cn } from "@/lib/utils"

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
  searchQuery: string
  selectedCount: number
  setSearchQuery: (query: string) => void
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
  searchQuery,
  selectedCount,
  setSearchQuery,
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
        {isSearchOpen && (
          <div className="sftp-search-box">
            <Search className="sftp-search-icon" />
            <Input
              ref={searchInputRef}
              value={searchQuery}
              onChange={(event) => setSearchQuery(event.target.value)}
              placeholder={t("sftp.search.placeholder", { defaultValue: "Filter current folder" })}
              disabled={!listingCurrentPath || isLoading}
              className="sftp-search-input"
              aria-label={t("sftp.search.label", { defaultValue: "Filter current folder" })}
            />
            {searchQuery && (
              <Button
                variant="ghost"
                size="icon-sm"
                onClick={() => setSearchQuery("")}
                className="sftp-search-clear"
                title={t("sftp.search.clear", { defaultValue: "Clear filter" })}
                aria-label={t("sftp.search.clear", { defaultValue: "Clear filter" })}
              >
                <X className="size-3" />
              </Button>
            )}
          </div>
        )}
      </div>
      <div className="sftp-header-actions">
        <Button
          variant={isSearchOpen || searchQuery ? "secondary" : "ghost"}
          size="icon-sm"
          onClick={handleToggleSearch}
          disabled={!listingCurrentPath || isLoading}
          title={t("sftp.search.label", { defaultValue: "Filter current folder" })}
          aria-label={t("sftp.search.label", { defaultValue: "Filter current folder" })}
          aria-pressed={isSearchOpen || Boolean(searchQuery)}
        >
          <Search className="size-4" />
        </Button>
        {selectedCount > 0 && (
          <>
            <Button
              variant="ghost"
              size="icon-sm"
              onClick={clearSelection}
              title={t("sftp.selection.clear", { defaultValue: "Clear selection" })}
            >
              <ListX className="size-4" />
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              onClick={handleDeleteSelection}
              title={t("sftp.actions.deleteSelected", { defaultValue: "Delete Selected" })}
            >
              <Trash2 className="size-4" />
            </Button>
          </>
        )}
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={handleUploadDialog}
          disabled={!listingCurrentPath || isLoading}
          title={t("sftp.actions.uploadFiles", { defaultValue: "Upload Files" })}
        >
          <ArrowUpFromLine className="size-4" />
        </Button>
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={handleUploadFolderDialog}
          disabled={!listingCurrentPath || isLoading}
          title={t("sftp.actions.uploadFolder", { defaultValue: "Upload Folder" })}
        >
          <FolderPlus className="size-4" />
        </Button>
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={handleCreateDirectory}
          disabled={!listingCurrentPath || isLoading}
          title={t("sftp.actions.newFolder", { defaultValue: "New Folder" })}
        >
          <FolderPlus className="size-4" />
        </Button>
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={() => void loadDirectory(listingCurrentPath ?? null)}
          disabled={isLoading}
          title={t("sftp.actions.refresh", { defaultValue: "Refresh" })}
        >
          <RefreshCcw className={cn("size-4", isLoading && "animate-spin")} />
        </Button>
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={onClose}
          title={t("sftp.actions.close", { defaultValue: "Close" })}
        >
          <X className="size-4" />
        </Button>
      </div>
    </div>
  )
}
