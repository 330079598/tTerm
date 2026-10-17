import { useTranslation } from "react-i18next"
import React from "react"
import { Archive, FolderOpen, KeyRound, RefreshCw, Save } from "lucide-react"

import type { AutomaticBackupSettings, BusyAction } from "@/components/SettingsDialog/dataMigration"
import type { SelectionItem, SelectionKey } from "@/components/SettingsDialog/backupShared"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import { Spinner } from "@/components/ui/spinner"

interface AutomaticBackupCardProps {
  busyAction: BusyAction | null
  busy: boolean
  automaticSettings: AutomaticBackupSettings
  setAutomaticSettings: React.Dispatch<React.SetStateAction<AutomaticBackupSettings | null>>
  selectionItems: SelectionItem[]
  updateAutomaticSelection: (key: SelectionKey, checked: boolean) => void
  handleChooseAutomaticDirectory: () => Promise<void>
  handleSaveAutomaticSettings: () => Promise<void>
  handleBackupNow: () => Promise<void>
}

/** Automatic backups: on or off, where they go, how often, how many and what. */
export function AutomaticBackupCard({
  busyAction,
  busy,
  automaticSettings,
  setAutomaticSettings,
  selectionItems,
  updateAutomaticSelection,
  handleChooseAutomaticDirectory,
  handleSaveAutomaticSettings,
  handleBackupNow,
}: AutomaticBackupCardProps) {
  const { t } = useTranslation()

  return (
    <Card>
      <CardContent className="space-y-4 p-4">
        <div>
          <div className="flex items-center gap-2 text-sm font-medium">
            <RefreshCw size={16} />
            {t("dataMigration.automaticTitle")}
          </div>
          <p className="text-muted-foreground mt-1 text-xs">
            {t("dataMigration.automaticDescription")}
          </p>
        </div>

        <div className="grid gap-3 sm:grid-cols-2">
          <div>
            <Label htmlFor="automatic-frequency">{t("dataMigration.automaticFrequency")}</Label>
            <Select
              id="automatic-frequency"
              className="mt-1.5"
              value={automaticSettings.frequency}
              disabled={busy}
              onChange={(event) =>
                setAutomaticSettings({
                  ...automaticSettings,
                  frequency: event.target.value as AutomaticBackupSettings["frequency"],
                })
              }
            >
              <option value="off">{t("dataMigration.frequencyOff")}</option>
              <option value="daily">{t("dataMigration.frequencyDaily")}</option>
              <option value="weekly">{t("dataMigration.frequencyWeekly")}</option>
            </Select>
          </div>
          <div>
            <Label htmlFor="automatic-retention">{t("dataMigration.retentionCount")}</Label>
            <Input
              id="automatic-retention"
              className="mt-1.5"
              type="number"
              min={1}
              max={50}
              value={automaticSettings.retentionCount}
              disabled={busy}
              onChange={(event) =>
                setAutomaticSettings({
                  ...automaticSettings,
                  retentionCount: Number(event.target.value),
                })
              }
            />
          </div>
        </div>

        <div>
          <Label htmlFor="automatic-directory">{t("dataMigration.backupDirectory")}</Label>
          <div className="mt-1.5 flex gap-2">
            <Input
              id="automatic-directory"
              readOnly
              value={automaticSettings.directory}
              placeholder={t("dataMigration.defaultBackupDirectory")}
            />
            <Button
              type="button"
              variant="outline"
              size="icon"
              disabled={busy}
              onClick={handleChooseAutomaticDirectory}
              aria-label={t("dataMigration.chooseDirectory")}
            >
              <FolderOpen size={16} />
            </Button>
          </div>
        </div>

        <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
          {selectionItems
            .filter((item) => item.key !== "secrets")
            .map((item) => (
              <label
                key={item.key}
                className="border-border flex min-h-10 cursor-pointer items-center gap-3 rounded-md border px-3 py-2 text-sm"
              >
                <Checkbox
                  checked={automaticSettings.selection[item.key]}
                  disabled={busy}
                  onCheckedChange={(checked) => updateAutomaticSelection(item.key, checked)}
                />
                <span>{item.label}</span>
              </label>
            ))}
        </div>

        <Alert>
          <KeyRound size={16} className="absolute top-3.5 left-4" />
          <AlertDescription className="pl-6">
            {t("dataMigration.automaticSecretsExcluded")}
          </AlertDescription>
        </Alert>

        <div className="flex flex-wrap gap-2">
          <Button disabled={busy} onClick={handleSaveAutomaticSettings}>
            <Save size={16} />
            {t("common.save")}
          </Button>
          <Button variant="outline" disabled={busy} onClick={handleBackupNow}>
            {busyAction === "backupNow" ? <Spinner /> : <Archive size={16} />}
            {busyAction === "backupNow"
              ? t("dataMigration.backingUp")
              : t("dataMigration.backupNow")}
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}
