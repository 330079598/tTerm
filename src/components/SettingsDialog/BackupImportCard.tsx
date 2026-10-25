import { useTranslation } from "react-i18next"
import { relaunch } from "@tauri-apps/plugin-process"
import {
  AlertTriangle,
  ArrowLeft,
  CheckCircle2,
  Cloud,
  FileSearch,
  KeyRound,
  RefreshCw,
  Upload,
} from "lucide-react"

import type {
  BackupImportResult,
  BackupInspectResult,
  BusyAction,
  MigrationView,
} from "@/components/SettingsDialog/dataMigration"
import type { RemoteBackupEntry } from "@/components/SettingsDialog/webdavBackup"
import { saveTerminalContent } from "@/components/TerminalTab/savedTerminalContent"
import {
  type BackupSelection,
  type SelectionItem,
  type SelectionKey,
  formatFileSize,
} from "@/components/SettingsDialog/backupShared"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { isVaultKeyPath } from "@/lib/keyVault"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import { Badge } from "@/components/ui/badge"
import { Spinner } from "@/components/ui/spinner"

interface BackupImportCardProps {
  selection: BackupSelection
  importPassword: string
  setImportPassword: (value: string) => void
  showPassword: boolean
  busyAction: BusyAction | null
  busy: boolean
  importPath: string
  importSource: RemoteBackupEntry | null
  inspectResult: BackupInspectResult | null
  conflictStrategy: string
  setConflictStrategy: (value: string) => void
  importResult: BackupImportResult | null
  selectionItems: SelectionItem[]
  showView: (view: MigrationView) => void
  updateSelection: (key: SelectionKey, checked: boolean) => void
  handleChooseImport: () => Promise<void>
  handleVerifyPassword: () => Promise<void>
  handleImport: () => Promise<void>
}

/**
 * Importing a backup file: choosing it, its password, what it holds next to
 * what is here, and how conflicts are settled.
 */
export function BackupImportCard({
  selection,
  importPassword,
  setImportPassword,
  showPassword,
  busyAction,
  busy,
  importPath,
  importSource,
  inspectResult,
  conflictStrategy,
  setConflictStrategy,
  importResult,
  selectionItems,
  showView,
  updateSelection,
  handleChooseImport,
  handleVerifyPassword,
  handleImport,
}: BackupImportCardProps) {
  const { t } = useTranslation()

  return (
    <Card>
      <CardContent className="space-y-4 p-4">
        <div>
          <div className="flex items-center gap-2 text-sm font-medium">
            <Upload size={16} />
            {t("dataMigration.importTitle")}
          </div>
          <p className="text-muted-foreground mt-1 text-xs">
            {t("dataMigration.importDescription")}
          </p>
        </div>

        <Button type="button" variant="outline" disabled={busy} onClick={handleChooseImport}>
          {busyAction === "inspect" ? <Spinner /> : <FileSearch size={16} />}
          {busyAction === "inspect"
            ? t("dataMigration.inspecting")
            : t("dataMigration.chooseBackup")}
        </Button>
        {importSource ? (
          <div className="border-border flex items-center justify-between gap-3 rounded-md border p-3">
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                <Cloud size={14} className="text-muted-foreground shrink-0" />
                <span className="truncate text-sm font-medium" title={importSource.fileName}>
                  {importSource.fileName}
                </span>
                {importSource.currentDevice ? (
                  <Badge variant="secondary">{t("dataMigration.webdav.thisDevice")}</Badge>
                ) : (
                  importSource.device && <Badge variant="outline">{importSource.device}</Badge>
                )}
              </div>
              <div className="text-muted-foreground mt-1 text-xs">
                {t("dataMigration.webdav.importSource")}
                {importSource.createdAt
                  ? ` · ${new Date(importSource.createdAt).toLocaleString()}`
                  : ""}
                {` · ${formatFileSize(importSource.sizeBytes)}`}
              </div>
            </div>
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="shrink-0"
              disabled={busy}
              onClick={() => showView("webdav")}
            >
              <ArrowLeft size={14} />
              {t("dataMigration.webdav.backToBackups")}
            </Button>
          </div>
        ) : (
          importPath && <p className="text-muted-foreground text-xs break-all">{importPath}</p>
        )}

        {inspectResult && (
          <>
            <div className="bg-muted/40 grid gap-2 rounded-md p-3 text-xs sm:grid-cols-2">
              <span>
                {t("dataMigration.createdAt")}:{" "}
                {new Date(inspectResult.manifest.createdAt).toLocaleString()}
              </span>
              <span>
                {t("dataMigration.sourceVersion")}: {inspectResult.manifest.appVersion}
              </span>
              <span>
                {t("dataMigration.profileCount")}: {inspectResult.profileCount}
              </span>
              <span>
                {t("dataMigration.secretCount")}: {inspectResult.secretCount}
              </span>
              <span>
                {t("dataMigration.profileChanges", {
                  added: inspectResult.diff.profiles.added,
                  updated: inspectResult.diff.profiles.updated,
                })}
              </span>
              <span>
                {t("dataMigration.commandChanges", {
                  added: inspectResult.diff.commands.added,
                  updated: inspectResult.diff.commands.updated,
                })}
              </span>
              {inspectResult.logFileCount > 0 && (
                <span>
                  {t("dataMigration.logFileCount", { count: inspectResult.logFileCount })}
                </span>
              )}
              {inspectResult.diff.settingsChanged && (
                <span>{t("dataMigration.settingsWillChange")}</span>
              )}
              {inspectResult.webdavPasswordUsed && (
                <span className="flex items-center gap-1">
                  <KeyRound size={12} />
                  {t("dataMigration.webdav.savedPasswordUsed")}
                </span>
              )}
            </div>

            {inspectResult.requiresPassword && !inspectResult.passwordVerified && (
              <div className="space-y-2">
                <Label htmlFor="import-backup-password">{t("dataMigration.backupPassword")}</Label>
                <div className="flex gap-2">
                  <Input
                    id="import-backup-password"
                    type={showPassword ? "text" : "password"}
                    value={importPassword}
                    disabled={busy}
                    onChange={(event) => setImportPassword(event.target.value)}
                  />
                  <Button
                    variant="outline"
                    disabled={busy || !importPassword}
                    onClick={handleVerifyPassword}
                  >
                    {busyAction === "verify" && <Spinner />}
                    {busyAction === "verify"
                      ? t("dataMigration.verifying")
                      : t("dataMigration.verify")}
                  </Button>
                </div>
              </div>
            )}

            <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
              {selectionItems.map((item) => (
                <label
                  key={item.key}
                  className="border-border flex min-h-10 items-center gap-3 rounded-md border px-3 py-2 text-sm"
                >
                  <Checkbox
                    checked={selection[item.key]}
                    disabled={busy || !inspectResult.manifest.selection[item.key]}
                    onCheckedChange={(checked) => updateSelection(item.key, checked)}
                  />
                  <span>{item.label}</span>
                </label>
              ))}
            </div>

            {selection.settings && inspectResult.diff.webdavAccount && (
              <Alert className="border-amber-500/40 bg-amber-500/10">
                <AlertTriangle size={16} className="absolute top-3.5 left-4 text-amber-500" />
                <div className="space-y-1 pl-6">
                  <AlertTitle>{t("dataMigration.webdavAccountChangeTitle")}</AlertTitle>
                  <AlertDescription className="space-y-1">
                    <p className="break-all">
                      {inspectResult.diff.webdavAccount.from ??
                        t("dataMigration.webdavAccountNone")}
                      {" → "}
                      {inspectResult.diff.webdavAccount.to}
                    </p>
                    <p>{t("dataMigration.webdavAccountChangeDescription")}</p>
                  </AlertDescription>
                </div>
              </Alert>
            )}

            <div className="grid gap-3 sm:grid-cols-2">
              <div>
                <Label htmlFor="migration-conflict">{t("dataMigration.conflictStrategy")}</Label>
                <Select
                  id="migration-conflict"
                  className="mt-1.5"
                  value={conflictStrategy}
                  disabled={busy}
                  onChange={(event) => setConflictStrategy(event.target.value)}
                >
                  <option value="merge">{t("dataMigration.merge")}</option>
                  <option value="replace">{t("dataMigration.replace")}</option>
                </Select>
              </div>
            </div>

            <Button
              disabled={
                busy ||
                !Object.values(selection).some(Boolean) ||
                (inspectResult.requiresPassword && !inspectResult.passwordVerified)
              }
              onClick={handleImport}
            >
              {busyAction === "import" ? <Spinner /> : <Upload size={16} />}
              {busyAction === "import"
                ? t("dataMigration.importing")
                : t("dataMigration.importAction")}
            </Button>
          </>
        )}

        {importResult && (
          <Alert className="border-emerald-500/40 bg-emerald-500/10">
            <CheckCircle2 size={16} className="absolute top-3.5 left-4 text-emerald-500" />
            <div className="space-y-3 pl-6">
              <div>
                <AlertTitle>{t("dataMigration.importSuccess")}</AlertTitle>
                <AlertDescription>
                  {t("dataMigration.importSummary", {
                    profiles: importResult.profilesImported,
                    commands: importResult.commandsImported,
                    secrets: importResult.secretsImported,
                  })}
                </AlertDescription>
              </div>
              {importResult.missingKeyFiles.length > 0 && (
                <div className="text-sm">
                  <div className="flex items-center gap-1.5 font-medium">
                    <AlertTriangle size={14} className="text-amber-500" />
                    {t("dataMigration.missingKeyFiles")}
                  </div>
                  <p className="text-muted-foreground mt-1 text-xs">
                    {t("dataMigration.missingKeyFilesDesc")}
                  </p>
                  <ul className="mt-2 space-y-1 text-xs">
                    {importResult.missingKeyFiles.map((file) => (
                      <li key={file.path} className="break-all">
                        {isVaultKeyPath(file.path) ? (
                          t("keyVault.missingFromVault")
                        ) : (
                          <code>{file.path}</code>
                        )}
                        <span className="text-muted-foreground">
                          {" · "}
                          {file.profiles.join(", ")}
                        </span>
                      </li>
                    ))}
                  </ul>
                </div>
              )}
              {importResult.requiresRestart && (
                <Button size="sm" onClick={() => void saveTerminalContent().then(relaunch)}>
                  <RefreshCw size={14} />
                  {t("dataMigration.restartNow")}
                </Button>
              )}
            </div>
          </Alert>
        )}
      </CardContent>
    </Card>
  )
}
