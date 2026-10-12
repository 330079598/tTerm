import { useTranslation } from "react-i18next"
import React from "react"
import { Download, Loader2, RefreshCw, RotateCcw, Trash2 } from "lucide-react"

import type { BusyAction, RemoteBackupEntry } from "@/components/SettingsDialog/webdavBackup"
import { formatFileSize } from "@/components/SettingsDialog/backupShared"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { Checkbox } from "@/components/ui/checkbox"

interface RemoteBackupsCardProps {
  busy: boolean
  busyAction: BusyAction | null
  remoteBackups: RemoteBackupEntry[] | null
  listError: string | null
  rowAction: { fileName: string; kind: "restore" | "download" } | null
  selected: Set<string>
  setSelected: React.Dispatch<React.SetStateAction<Set<string>>>
  configured: boolean
  vaultLocked: boolean
  allSelected: boolean
  loadRemoteBackups: (prompt?: boolean) => Promise<void>
  handleRestore: (entry: RemoteBackupEntry) => Promise<void>
  handleDownload: (entry: RemoteBackupEntry) => Promise<void>
  handleDelete: (entry: RemoteBackupEntry) => void
  handleDeleteSelected: () => void
  toggleSelected: (fileName: string, checked: boolean) => void
}

/** The backups on the server: restoring, downloading and deleting them. */
export function RemoteBackupsCard({
  busy,
  busyAction,
  remoteBackups,
  listError,
  rowAction,
  selected,
  setSelected,
  configured,
  vaultLocked,
  allSelected,
  loadRemoteBackups,
  handleRestore,
  handleDownload,
  handleDelete,
  handleDeleteSelected,
  toggleSelected,
}: RemoteBackupsCardProps) {
  const { t } = useTranslation()

  return (
    <Card>
      <CardContent className="space-y-4 p-4">
        <div className="flex items-start justify-between gap-3">
          <div>
            <div className="text-sm font-medium">{t("dataMigration.webdav.remoteTitle")}</div>
            <p className="text-muted-foreground mt-1 text-xs">
              {t("dataMigration.webdav.remoteDescription")}
            </p>
          </div>
          <Button
            type="button"
            variant="ghost"
            size="icon"
            disabled={busy || !configured}
            onClick={() => void loadRemoteBackups()}
            aria-label={t("dataMigration.webdav.refresh")}
          >
            <RefreshCw size={16} className={busyAction === "list" ? "animate-spin" : ""} />
          </Button>
        </div>
        {!configured ? (
          <p className="text-muted-foreground text-sm">{t("dataMigration.webdav.notConfigured")}</p>
        ) : vaultLocked && remoteBackups === null ? (
          <p className="text-muted-foreground text-sm">{t("dataMigration.webdav.lockedList")}</p>
        ) : listError ? (
          <p className="text-destructive text-sm break-all">{listError}</p>
        ) : remoteBackups === null ? (
          <p className="text-muted-foreground text-sm">{t("dataMigration.webdav.loading")}</p>
        ) : remoteBackups.length === 0 ? (
          <p className="text-muted-foreground text-sm">{t("dataMigration.webdav.empty")}</p>
        ) : (
          <div className="space-y-2">
            <div className="flex items-center justify-between gap-3">
              <label className="flex cursor-pointer items-center gap-2 text-sm">
                <Checkbox
                  checked={allSelected}
                  disabled={busy}
                  onCheckedChange={(checked) =>
                    setSelected(
                      new Set(checked ? remoteBackups.map((entry) => entry.fileName) : [])
                    )
                  }
                  aria-label={t("dataMigration.webdav.selectAll")}
                />
                <span className="text-muted-foreground">
                  {t("dataMigration.webdav.selectedCount", {
                    selected: selected.size,
                    total: remoteBackups.length,
                  })}
                </span>
              </label>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="text-destructive"
                disabled={busy || selected.size === 0}
                onClick={handleDeleteSelected}
              >
                {busyAction === "delete" ? (
                  <Loader2 className="animate-spin" size={16} />
                ) : (
                  <Trash2 size={16} />
                )}
                {t("dataMigration.webdav.deleteSelected", { count: selected.size })}
              </Button>
            </div>
            <div className="max-h-80 space-y-2 overflow-y-auto pr-1">
              {remoteBackups.map((entry) => (
                <div
                  key={entry.fileName}
                  className="border-border flex items-center justify-between gap-3 rounded-md border p-3"
                >
                  <div className="flex min-w-0 items-center gap-3">
                    <Checkbox
                      checked={selected.has(entry.fileName)}
                      disabled={busy}
                      onCheckedChange={(checked) => toggleSelected(entry.fileName, checked)}
                      aria-label={t("dataMigration.webdav.selectBackup", {
                        name: entry.fileName,
                      })}
                    />
                    <div className="min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="truncate text-sm font-medium" title={entry.fileName}>
                          {entry.fileName}
                        </span>
                        {entry.currentDevice ? (
                          <Badge variant="secondary">{t("dataMigration.webdav.thisDevice")}</Badge>
                        ) : (
                          entry.device && <Badge variant="outline">{entry.device}</Badge>
                        )}
                      </div>
                      <div className="text-muted-foreground mt-1 text-xs">
                        {entry.createdAt ? `${new Date(entry.createdAt).toLocaleString()} · ` : ""}
                        {formatFileSize(entry.sizeBytes)}
                      </div>
                    </div>
                  </div>
                  <div className="flex shrink-0 gap-1">
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon"
                      disabled={busy}
                      onClick={() => void handleDownload(entry)}
                      title={t("dataMigration.webdav.downloadBackup", { name: entry.fileName })}
                      aria-label={t("dataMigration.webdav.downloadBackup", {
                        name: entry.fileName,
                      })}
                    >
                      {rowAction?.fileName === entry.fileName && rowAction.kind === "download" ? (
                        <Loader2 className="animate-spin" size={16} />
                      ) : (
                        <Download size={16} />
                      )}
                    </Button>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon"
                      disabled={busy}
                      onClick={() => void handleRestore(entry)}
                      title={t("dataMigration.webdav.restoreBackup", { name: entry.fileName })}
                      aria-label={t("dataMigration.webdav.restoreBackup", {
                        name: entry.fileName,
                      })}
                    >
                      {rowAction?.fileName === entry.fileName && rowAction.kind === "restore" ? (
                        <Loader2 className="animate-spin" size={16} />
                      ) : (
                        <RotateCcw size={16} />
                      )}
                    </Button>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon"
                      disabled={busy}
                      onClick={() => handleDelete(entry)}
                      title={t("dataMigration.deleteBackup", { name: entry.fileName })}
                      aria-label={t("dataMigration.deleteBackup", { name: entry.fileName })}
                    >
                      <Trash2 size={16} />
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  )
}
