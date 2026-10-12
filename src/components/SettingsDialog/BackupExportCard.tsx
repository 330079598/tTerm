import { useTranslation } from "react-i18next"
import React from "react"
import { CheckCircle2, Download, Eye, EyeOff, KeyRound, Loader2 } from "lucide-react"

import type { BackupExportResult, BusyAction } from "@/components/SettingsDialog/dataMigration"
import type {
  BackupSelection,
  SelectionItem,
  SelectionKey,
} from "@/components/SettingsDialog/backupShared"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"

interface BackupExportCardProps {
  selection: BackupSelection
  backupPassword: string
  confirmPassword: string
  setBackupPassword: (value: string) => void
  setConfirmPassword: (value: string) => void
  showPassword: boolean
  setShowPassword: React.Dispatch<React.SetStateAction<boolean>>
  busyAction: BusyAction | null
  busy: boolean
  exportResult: BackupExportResult | null
  selectionItems: SelectionItem[]
  updateSelection: (key: SelectionKey, checked: boolean) => void
  handleExport: () => Promise<void>
}

/** Making a backup file now: what goes in it and its password. */
export function BackupExportCard({
  selection,
  backupPassword,
  confirmPassword,
  setBackupPassword,
  setConfirmPassword,
  showPassword,
  setShowPassword,
  busyAction,
  busy,
  exportResult,
  selectionItems,
  updateSelection,
  handleExport,
}: BackupExportCardProps) {
  const { t } = useTranslation()

  return (
    <Card>
      <CardContent className="space-y-4 p-4">
        <div>
          <div className="flex items-center gap-2 text-sm font-medium">
            <Download size={16} />
            {t("dataMigration.exportTitle")}
          </div>
          <p className="text-muted-foreground mt-1 text-xs">
            {t("dataMigration.exportDescription")}
          </p>
        </div>

        <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
          {selectionItems.map((item) => (
            <label
              key={item.key}
              className="border-border flex min-h-10 cursor-pointer items-center gap-3 rounded-md border px-3 py-2 text-sm"
            >
              <Checkbox
                checked={selection[item.key]}
                disabled={busy}
                onCheckedChange={(checked) => updateSelection(item.key, checked)}
              />
              <span>{item.label}</span>
            </label>
          ))}
        </div>

        {selection.secrets && (
          <Alert className="border-amber-500/40 bg-amber-500/10">
            <KeyRound size={16} className="absolute top-3.5 left-4" />
            <div className="pl-6">
              <AlertTitle>{t("dataMigration.encryptedSecrets")}</AlertTitle>
              <AlertDescription>{t("dataMigration.encryptedSecretsDesc")}</AlertDescription>
            </div>
          </Alert>
        )}

        {(selection.secrets || backupPassword) && (
          <div className="grid gap-3 sm:grid-cols-2">
            <div>
              <Label htmlFor="backup-password">{t("dataMigration.backupPassword")}</Label>
              <div className="relative mt-1.5">
                <Input
                  id="backup-password"
                  type={showPassword ? "text" : "password"}
                  value={backupPassword}
                  disabled={busy}
                  onChange={(event) => setBackupPassword(event.target.value)}
                  className="pr-10"
                />
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  className="absolute top-0 right-0"
                  onClick={() => setShowPassword((visible) => !visible)}
                  aria-label={t("dataMigration.togglePassword")}
                >
                  {showPassword ? <EyeOff size={16} /> : <Eye size={16} />}
                </Button>
              </div>
            </div>
            <div>
              <Label htmlFor="backup-password-confirm">{t("dataMigration.confirmPassword")}</Label>
              <Input
                id="backup-password-confirm"
                className="mt-1.5"
                type={showPassword ? "text" : "password"}
                value={confirmPassword}
                disabled={busy}
                onChange={(event) => setConfirmPassword(event.target.value)}
              />
            </div>
          </div>
        )}

        <Button disabled={busy || !Object.values(selection).some(Boolean)} onClick={handleExport}>
          {busyAction === "export" ? (
            <Loader2 className="animate-spin" size={16} />
          ) : (
            <Download size={16} />
          )}
          {busyAction === "export" ? t("dataMigration.exporting") : t("dataMigration.exportAction")}
        </Button>

        {exportResult && (
          <Alert>
            <CheckCircle2 size={16} className="absolute top-3.5 left-4 text-emerald-500" />
            <div className="pl-6">
              <AlertTitle>{t("dataMigration.exportSuccess")}</AlertTitle>
              <AlertDescription className="break-all">
                {exportResult.outputPath}
                <br />
                {t("dataMigration.exportSummary", {
                  profiles: exportResult.profileCount,
                  commands: exportResult.commandCount,
                  secrets: exportResult.secretCount,
                })}
              </AlertDescription>
            </div>
          </Alert>
        )}
      </CardContent>
    </Card>
  )
}
