import React from "react"
import { FolderTree, Square, X } from "lucide-react"
import { useTranslation } from "react-i18next"

import type { SftpRecursiveSearch } from "@/components/SftpDrawer/useSftpRecursiveSearch"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"

interface SftpSearchStatusProps {
  search: SftpRecursiveSearch | null
  onStop: () => void
  /** Leaves the results for the folder's own listing. */
  onClose: () => void
}

/** What a search of subfolders is doing, above the results it found. */
export const SftpSearchStatus: React.FC<SftpSearchStatusProps> = ({ search, onStop, onClose }) => {
  const { t } = useTranslation()
  if (!search) return null

  const count = search.entries.length
  const notes: string[] = []
  if (search.summary?.truncated) notes.push(t("sftp.search.truncated", { count }))
  if (search.summary?.cancelled) notes.push(t("sftp.search.stopped"))
  if (search.summary?.method === "sftp") notes.push(t("sftp.search.usedSftp"))
  if (search.summary?.skippedFolders) {
    notes.push(t("sftp.search.skippedFolders", { count: search.summary.skippedFolders }))
  }

  return (
    <div className="sftp-search-status" role="status">
      {search.status === "searching" ? (
        <Spinner />
      ) : (
        <FolderTree className="text-muted-foreground size-3.5 shrink-0" />
      )}
      <div className="min-w-0 flex-1 truncate">
        {search.status === "failed" ? (
          <span className="text-destructive">{search.error}</span>
        ) : (
          <>
            <span>
              {search.status === "searching"
                ? t("sftp.search.searching", { count, query: search.query })
                : t("sftp.search.found", { count, query: search.query })}
            </span>
            {search.status === "searching" && search.scannedFolders > 0 && (
              <span className="text-muted-foreground">
                {" · "}
                {t("sftp.search.scannedFolders", { count: search.scannedFolders })}
              </span>
            )}
            {notes.length > 0 && (
              <span className="text-muted-foreground">
                {" · "}
                {notes.join(" · ")}
              </span>
            )}
          </>
        )}
      </div>
      {search.status === "searching" ? (
        <Button variant="ghost" size="xs" onClick={onStop}>
          <Square className="size-3" />
          {t("sftp.search.stop")}
        </Button>
      ) : (
        <Button
          variant="ghost"
          size="icon-xs"
          onClick={onClose}
          aria-label={t("sftp.search.closeResults")}
          title={t("sftp.search.closeResults")}
        >
          <X className="size-3" />
        </Button>
      )}
    </div>
  )
}
