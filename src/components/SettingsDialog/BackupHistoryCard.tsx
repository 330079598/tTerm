import { useTranslation } from "react-i18next"
import { Trash2, Upload } from "lucide-react"

import type { BackupHistoryEntry } from "@/components/SettingsDialog/dataMigration"
import type { RemoteBackupEntry } from "@/components/SettingsDialog/webdavBackup"
import { formatFileSize } from "@/components/SettingsDialog/backupShared"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"

interface BackupHistoryCardProps {
  busy: boolean
  backupHistory: BackupHistoryEntry[]
  handleDeleteHistory: (entry: BackupHistoryEntry) => Promise<void>
  openForImport: (path: string, source?: RemoteBackupEntry | null) => Promise<void>
}

/** The backups made so far, automatic or before an import, and deleting them. */
export function BackupHistoryCard({
  busy,
  backupHistory,
  handleDeleteHistory,
  openForImport,
}: BackupHistoryCardProps) {
  const { t } = useTranslation()

  return (
    <Card>
      <CardContent className="space-y-4 p-4">
        <div>
          <div className="text-sm font-medium">{t("dataMigration.historyTitle")}</div>
          <p className="text-muted-foreground mt-1 text-xs">
            {t("dataMigration.historyDescription")}
          </p>
        </div>
        {backupHistory.length === 0 ? (
          <p className="text-muted-foreground text-sm">{t("dataMigration.historyEmpty")}</p>
        ) : (
          <div className="space-y-2">
            {backupHistory.map((entry) => (
              <div
                key={entry.path}
                className="border-border flex items-center justify-between gap-3 rounded-md border p-3"
              >
                <div className="min-w-0">
                  <div className="truncate text-sm font-medium" title={entry.fileName}>
                    {entry.fileName}
                  </div>
                  <div className="text-muted-foreground mt-1 text-xs">
                    {new Date(entry.modifiedAt).toLocaleString()} ·{" "}
                    {formatFileSize(entry.sizeBytes)}
                  </div>
                </div>
                <div className="flex shrink-0 gap-1">
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    disabled={busy}
                    onClick={() => openForImport(entry.path)}
                    aria-label={t("dataMigration.useBackup", { name: entry.fileName })}
                  >
                    <Upload size={16} />
                  </Button>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    disabled={busy}
                    onClick={() => handleDeleteHistory(entry)}
                    aria-label={t("dataMigration.deleteBackup", { name: entry.fileName })}
                  >
                    <Trash2 size={16} />
                  </Button>
                </div>
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  )
}
