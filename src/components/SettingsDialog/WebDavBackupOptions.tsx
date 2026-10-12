import { useTranslation } from "react-i18next"
import { AlertTriangle, CheckCircle2 } from "lucide-react"

import type {
  WebDavBackupSettings,
  WebDavBackupStatus,
} from "@/components/SettingsDialog/webdavBackup"
import type { SelectionKey, SelectionItem } from "@/components/SettingsDialog/backupShared"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"

interface WebDavBackupOptionsProps {
  form: WebDavBackupSettings
  status: WebDavBackupStatus
  busy: boolean
  update: (patch: Partial<WebDavBackupSettings>) => void
  backupPassword: string
  confirmBackupPassword: string
  setBackupPassword: (value: string) => void
  setConfirmBackupPassword: (value: string) => void
  passwordType: "text" | "password"
  savedPlaceholder: string
  updateSelection: (key: SelectionKey, checked: boolean) => void
  selectionItems: SelectionItem[]
}

/**
 * How backups are made: the password they are encrypted with, how often
 * they run and how many are kept, what goes in them, and how the last run
 * went.
 */
export function WebDavBackupOptions({
  form,
  status,
  busy,
  update,
  backupPassword,
  confirmBackupPassword,
  setBackupPassword,
  setConfirmBackupPassword,
  passwordType,
  savedPlaceholder,
  updateSelection,
  selectionItems,
}: WebDavBackupOptionsProps) {
  const { t } = useTranslation()

  return (
    <>
      <div className="grid gap-3 sm:grid-cols-2">
        <div>
          <Label htmlFor="webdav-backup-password">{t("dataMigration.backupPassword")}</Label>
          <Input
            id="webdav-backup-password"
            className="mt-1.5"
            type={passwordType}
            value={backupPassword}
            disabled={busy}
            placeholder={status.hasBackupPassword ? savedPlaceholder : ""}
            onChange={(event) => setBackupPassword(event.target.value)}
          />
        </div>
        <div>
          <Label htmlFor="webdav-backup-password-confirm">
            {t("dataMigration.confirmPassword")}
          </Label>
          <Input
            id="webdav-backup-password-confirm"
            className="mt-1.5"
            type={passwordType}
            value={confirmBackupPassword}
            disabled={busy || !backupPassword}
            onChange={(event) => setConfirmBackupPassword(event.target.value)}
          />
        </div>
      </div>

      <div className="grid gap-3 sm:grid-cols-2">
        <div>
          <Label htmlFor="webdav-frequency">{t("dataMigration.automaticFrequency")}</Label>
          <Select
            id="webdav-frequency"
            className="mt-1.5"
            value={form.frequency}
            disabled={busy}
            onChange={(event) =>
              update({ frequency: event.target.value as WebDavBackupSettings["frequency"] })
            }
          >
            <option value="off">{t("dataMigration.frequencyOff")}</option>
            <option value="daily">{t("dataMigration.frequencyDaily")}</option>
            <option value="weekly">{t("dataMigration.frequencyWeekly")}</option>
          </Select>
        </div>
        <div>
          <Label htmlFor="webdav-retention">{t("dataMigration.retentionCount")}</Label>
          <Input
            id="webdav-retention"
            className="mt-1.5"
            type="number"
            min={1}
            max={50}
            value={form.retentionCount}
            disabled={busy}
            onChange={(event) => update({ retentionCount: Number(event.target.value) })}
          />
        </div>
      </div>
      <p className="text-muted-foreground -mt-2 text-xs">
        {t("dataMigration.webdav.retentionHint", { device: status.deviceName })}
      </p>

      <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
        {selectionItems.map((item) => (
          <label
            key={item.key}
            className="border-border flex min-h-10 cursor-pointer items-center gap-3 rounded-md border px-3 py-2 text-sm"
          >
            <Checkbox
              checked={form.selection[item.key]}
              disabled={busy}
              onCheckedChange={(checked) => updateSelection(item.key, checked)}
            />
            <span>{item.label}</span>
          </label>
        ))}
      </div>

      {status.settings.lastError ? (
        <Alert className="border-destructive/40 bg-destructive/10">
          <AlertTriangle size={16} className="absolute top-3.5 left-4" />
          <div className="pl-6">
            <AlertTitle>{t("dataMigration.webdav.lastFailed")}</AlertTitle>
            <AlertDescription className="break-all">{status.settings.lastError}</AlertDescription>
          </div>
        </Alert>
      ) : (
        status.settings.lastBackupAt && (
          <p className="text-muted-foreground flex items-center gap-1.5 text-xs">
            <CheckCircle2 size={14} className="text-emerald-500" />
            {t("dataMigration.webdav.lastBackupAt", {
              time: new Date(status.settings.lastBackupAt).toLocaleString(),
            })}
          </p>
        )
      )}
    </>
  )
}
