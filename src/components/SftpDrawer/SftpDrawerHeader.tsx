import React from "react"
import {
  ArrowUpFromLine,
  ChevronRight,
  FolderPlus,
  ListX,
  RefreshCcw,
  Trash2,
  X,
} from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
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
  selectedCount: number
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
  selectedCount,
}) => {
  const { t } = useTranslation()

  return (
    <div className="sftp-drawer-header">
      <div className="sftp-header-left">
        <span className="sftp-drawer-eyebrow">SFTP</span>
        <div className="flex items-center gap-1">
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
