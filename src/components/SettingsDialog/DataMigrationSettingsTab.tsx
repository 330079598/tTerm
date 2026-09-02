import React, { useCallback, useEffect, useMemo, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { open as openFileDialog, save as saveFileDialog } from "@tauri-apps/plugin-dialog"
import { relaunch } from "@tauri-apps/plugin-process"
import {
  Archive,
  CheckCircle2,
  Download,
  Eye,
  EyeOff,
  FileSearch,
  FolderOpen,
  History,
  KeyRound,
  RefreshCw,
  Save,
  Trash2,
  Upload,
} from "lucide-react"
import { useTranslation } from "react-i18next"

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Select } from "@/components/ui/select"
import { useToast } from "@/hooks/use-toast"
import { RECENT_COMMANDS_STORAGE_KEY } from "@/lib/recentCommands"
import { toErrorMessage } from "@/lib/utils"

interface BackupSelection {
  settings: boolean
  profiles: boolean
  session: boolean
  knownHosts: boolean
  sftpDirectories: boolean
  commandLibrary: boolean
  themes: boolean
  secrets: boolean
  logs: boolean
}

interface CategoryDiff {
  added: number
  updated: number
  unchanged: number
}

interface BackupManifest {
  appVersion: string
  createdAt: string
  platform: string
  selection: BackupSelection
  encrypted: boolean
  secretCount: number
}

interface BackupInspectResult {
  manifest: BackupManifest
  requiresPassword: boolean
  passwordVerified: boolean
  profileCount: number
  commandCount: number
  secretCount: number
  hasFrontendState: boolean
  logFileCount: number
  diff: {
    profiles: CategoryDiff
    commands: CategoryDiff
    settingsChanged: boolean
  }
}

interface AutomaticBackupSettings {
  frequency: "off" | "daily" | "weekly"
  directory: string
  retentionCount: number
  selection: BackupSelection
  lastBackupAt: number | null
}

interface BackupHistoryEntry {
  path: string
  fileName: string
  sizeBytes: number
  modifiedAt: number
  kind: "automatic" | "recovery" | "manual"
}

interface BackupExportResult {
  outputPath: string
  profileCount: number
  commandCount: number
  secretCount: number
  encrypted: boolean
}

interface BackupImportResult {
  profilesImported: number
  commandsImported: number
  secretsImported: number
  secretDestination: "system" | "vault" | "hybrid" | null
  frontendState: {
    customThemes?: unknown[]
    recentCommands?: unknown[]
    sftpColumnWidths?: unknown
  } | null
  preImportBackupPath: string
  requiresRestart: boolean
}

type SelectionKey = keyof BackupSelection
type MigrationView = "backup" | "import" | "history"

const defaultSelection: BackupSelection = {
  settings: true,
  profiles: true,
  session: true,
  knownHosts: true,
  sftpDirectories: true,
  commandLibrary: true,
  themes: true,
  secrets: false,
  logs: false,
}

function cloneAvailableSelection(selection: BackupSelection): BackupSelection {
  return { ...selection }
}

function readFrontendState() {
  const read = (key: string, fallback: unknown) => {
    try {
      return JSON.parse(localStorage.getItem(key) ?? JSON.stringify(fallback)) as unknown
    } catch {
      return fallback
    }
  }
  const customThemes = read("custom-themes", [])
  const recentCommands = read(RECENT_COMMANDS_STORAGE_KEY, [])
  try {
    return {
      customThemes: Array.isArray(customThemes) ? customThemes : [],
      recentCommands: Array.isArray(recentCommands) ? recentCommands : [],
      sftpColumnWidths: read("tterm.sftp.columnWidths", null),
    }
  } catch {
    return { customThemes: [], recentCommands: [], sftpColumnWidths: null }
  }
}

function restoreFrontendState(state: BackupImportResult["frontendState"]) {
  if (state && Array.isArray(state.customThemes)) {
    localStorage.setItem("custom-themes", JSON.stringify(state.customThemes))
  }
  if (state && Array.isArray(state.recentCommands)) {
    localStorage.setItem(RECENT_COMMANDS_STORAGE_KEY, JSON.stringify(state.recentCommands))
  }
  if (state && state.sftpColumnWidths !== undefined && state.sftpColumnWidths !== null) {
    localStorage.setItem("tterm.sftp.columnWidths", JSON.stringify(state.sftpColumnWidths))
  }
}

function formatFileSize(bytes: number) {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}

export const DataMigrationSettingsTab: React.FC = () => {
  const { t } = useTranslation()
  const { toast } = useToast()
  const [selection, setSelection] = useState<BackupSelection>(defaultSelection)
  const [backupPassword, setBackupPassword] = useState("")
  const [confirmPassword, setConfirmPassword] = useState("")
  const [showPassword, setShowPassword] = useState(false)
  const [busy, setBusy] = useState(false)
  const [activeView, setActiveView] = useState<MigrationView>("backup")
  const [importPath, setImportPath] = useState("")
  const [inspectResult, setInspectResult] = useState<BackupInspectResult | null>(null)
  const [conflictStrategy, setConflictStrategy] = useState("merge")
  const [secretDestination, setSecretDestination] = useState("auto")
  const [exportResult, setExportResult] = useState<BackupExportResult | null>(null)
  const [importResult, setImportResult] = useState<BackupImportResult | null>(null)
  const [automaticSettings, setAutomaticSettings] = useState<AutomaticBackupSettings | null>(null)
  const [backupHistory, setBackupHistory] = useState<BackupHistoryEntry[]>([])

  const selectionItems = useMemo(
    () => [
      { key: "settings" as const, label: t("dataMigration.categories.settings") },
      { key: "profiles" as const, label: t("dataMigration.categories.profiles") },
      { key: "session" as const, label: t("dataMigration.categories.session") },
      { key: "knownHosts" as const, label: t("dataMigration.categories.knownHosts") },
      {
        key: "sftpDirectories" as const,
        label: t("dataMigration.categories.sftpDirectories"),
      },
      { key: "commandLibrary" as const, label: t("dataMigration.categories.commands") },
      { key: "themes" as const, label: t("dataMigration.categories.themes") },
      { key: "logs" as const, label: t("dataMigration.categories.logs") },
      { key: "secrets" as const, label: t("dataMigration.categories.secrets") },
    ],
    [t]
  )

  const refreshBackupManagement = useCallback(async () => {
    const [settings, history] = await Promise.all([
      invoke<AutomaticBackupSettings>("get_automatic_backup_settings"),
      invoke<BackupHistoryEntry[]>("list_backup_history"),
    ])
    setAutomaticSettings(settings)
    setBackupHistory(history)
  }, [])

  useEffect(() => {
    refreshBackupManagement().catch((error) => {
      console.error("Failed to load backup management settings:", error)
    })
  }, [refreshBackupManagement])

  const updateSelection = (key: SelectionKey, checked: boolean) => {
    setSelection((current) => {
      const next = { ...current, [key]: checked }
      if (key === "secrets" && checked) next.profiles = true
      if (key === "profiles" && !checked) next.secrets = false
      return next
    })
  }

  const updateAutomaticSelection = (key: SelectionKey, checked: boolean) => {
    if (!automaticSettings || key === "secrets") return
    setAutomaticSettings({
      ...automaticSettings,
      selection: { ...automaticSettings.selection, [key]: checked, secrets: false },
    })
  }

  const handleExport = async () => {
    if (!Object.values(selection).some(Boolean)) return
    if (selection.secrets && backupPassword.length < 8) {
      toast({
        title: t("dataMigration.passwordRequired"),
        description: t("dataMigration.passwordMinimum"),
        variant: "destructive",
      })
      return
    }
    if (backupPassword !== confirmPassword) {
      toast({
        title: t("dataMigration.passwordMismatch"),
        variant: "destructive",
      })
      return
    }
    const outputPath = await saveFileDialog({
      defaultPath: `tterm-backup-${new Date().toISOString().slice(0, 10)}.tterm-backup`,
      filters: [{ name: "tTerm Backup", extensions: ["tterm-backup"] }],
    })
    if (!outputPath) return

    setBusy(true)
    setExportResult(null)
    try {
      const result = await invoke<BackupExportResult>("export_backup", {
        outputPath,
        options: {
          selection,
          backupPassword: backupPassword || null,
          frontendState: readFrontendState(),
        },
      })
      setExportResult(result)
      setBackupPassword("")
      setConfirmPassword("")
      toast({ title: t("dataMigration.exportSuccess") })
    } catch (error) {
      toast({
        title: t("dataMigration.exportFailed"),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    } finally {
      setBusy(false)
    }
  }

  const inspect = async (path: string, password: string) => {
    const result = await invoke<BackupInspectResult>("inspect_backup", {
      input: { inputPath: path, backupPassword: password || null },
    })
    setInspectResult(result)
    setSelection(cloneAvailableSelection(result.manifest.selection))
    return result
  }

  const handleChooseImport = async () => {
    const selected = await openFileDialog({
      multiple: false,
      filters: [{ name: "tTerm Backup", extensions: ["tterm-backup"] }],
    })
    if (!selected || Array.isArray(selected)) return
    setBusy(true)
    setImportPath(selected)
    setBackupPassword("")
    setImportResult(null)
    try {
      await inspect(selected, "")
    } catch (error) {
      setInspectResult(null)
      toast({
        title: t("dataMigration.inspectFailed"),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    } finally {
      setBusy(false)
    }
  }

  const handleVerifyPassword = async () => {
    if (!importPath) return
    setBusy(true)
    try {
      await inspect(importPath, backupPassword)
      toast({ title: t("dataMigration.passwordVerified") })
    } catch (error) {
      toast({
        title: t("dataMigration.passwordInvalid"),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    } finally {
      setBusy(false)
    }
  }

  const handleImport = async () => {
    if (!importPath || !inspectResult || !Object.values(selection).some(Boolean)) return
    if (inspectResult.requiresPassword && !inspectResult.passwordVerified) {
      await handleVerifyPassword()
      return
    }
    if (!window.confirm(t("dataMigration.importConfirm"))) return

    setBusy(true)
    try {
      const result = await invoke<BackupImportResult>("import_backup", {
        inputPath: importPath,
        options: {
          selection,
          backupPassword: backupPassword || null,
          conflictStrategy,
          secretDestination,
        },
      })
      restoreFrontendState(result.frontendState)
      setImportResult(result)
      setBackupPassword("")
      setConfirmPassword("")
      toast({ title: t("dataMigration.importSuccess") })
    } catch (error) {
      toast({
        title: t("dataMigration.importFailed"),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    } finally {
      setBusy(false)
    }
  }

  const handleChooseAutomaticDirectory = async () => {
    if (!automaticSettings) return
    const directory = await openFileDialog({ directory: true, multiple: false })
    if (!directory || Array.isArray(directory)) return
    setAutomaticSettings({ ...automaticSettings, directory })
  }

  const handleSaveAutomaticSettings = async () => {
    if (!automaticSettings) return
    setBusy(true)
    try {
      const saved = await invoke<AutomaticBackupSettings>("save_automatic_backup_settings", {
        settings: automaticSettings,
      })
      setAutomaticSettings(saved)
      toast({ title: t("dataMigration.automaticSaved") })
    } catch (error) {
      toast({
        title: t("dataMigration.automaticSaveFailed"),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    } finally {
      setBusy(false)
    }
  }

  const handleBackupNow = async () => {
    if (!automaticSettings) return
    setBusy(true)
    try {
      await invoke("save_automatic_backup_settings", { settings: automaticSettings })
      await invoke("run_due_automatic_backup", {
        frontendState: readFrontendState(),
        force: true,
      })
      await refreshBackupManagement()
      toast({ title: t("dataMigration.backupNowSuccess") })
    } catch (error) {
      toast({
        title: t("dataMigration.backupNowFailed"),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    } finally {
      setBusy(false)
    }
  }

  const handleDeleteHistory = async (entry: BackupHistoryEntry) => {
    if (!window.confirm(t("dataMigration.deleteBackupConfirm", { name: entry.fileName }))) return
    setBusy(true)
    try {
      await invoke("delete_backup_history_entry", { path: entry.path })
      await refreshBackupManagement()
    } catch (error) {
      toast({
        title: t("dataMigration.deleteBackupFailed"),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    } finally {
      setBusy(false)
    }
  }

  const handleUseHistory = async (entry: BackupHistoryEntry) => {
    setBusy(true)
    setActiveView("import")
    setImportPath(entry.path)
    setBackupPassword("")
    setImportResult(null)
    try {
      await inspect(entry.path, "")
    } catch (error) {
      toast({
        title: t("dataMigration.inspectFailed"),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    } finally {
      setBusy(false)
    }
  }

  return (
    <ScrollArea className="h-full pr-4">
      <div className="space-y-6">
        <div>
          <h3 className="flex items-center gap-2 text-sm font-semibold">
            <Archive size={16} />
            {t("dataMigration.title")}
          </h3>
          <p className="text-muted-foreground mt-1 text-xs leading-5">
            {t("dataMigration.description")}
          </p>
        </div>

        <div
          className="bg-muted/40 grid grid-cols-3 gap-1 rounded-md p-1"
          role="group"
          aria-label={t("dataMigration.title")}
        >
          <Button
            type="button"
            size="sm"
            variant={activeView === "backup" ? "default" : "ghost"}
            aria-pressed={activeView === "backup"}
            onClick={() => setActiveView("backup")}
          >
            <Archive size={14} />
            {t("dataMigration.backupTab", { defaultValue: "Backup" })}
          </Button>
          <Button
            type="button"
            size="sm"
            variant={activeView === "import" ? "default" : "ghost"}
            aria-pressed={activeView === "import"}
            onClick={() => setActiveView("import")}
          >
            <Upload size={14} />
            {t("dataMigration.importTab", { defaultValue: "Import" })}
          </Button>
          <Button
            type="button"
            size="sm"
            variant={activeView === "history" ? "default" : "ghost"}
            aria-pressed={activeView === "history"}
            onClick={() => setActiveView("history")}
          >
            <History size={14} />
            {t("dataMigration.historyTab", { defaultValue: "History" })}
          </Button>
        </div>

        {activeView === "backup" && automaticSettings && (
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
                  <Label htmlFor="automatic-frequency">
                    {t("dataMigration.automaticFrequency")}
                  </Label>
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
                  <Archive size={16} />
                  {t("dataMigration.backupNow")}
                </Button>
              </div>
            </CardContent>
          </Card>
        )}

        {activeView === "history" && (
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
                          onClick={() => handleUseHistory(entry)}
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
        )}

        {activeView === "backup" && (
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
                    <Label htmlFor="backup-password-confirm">
                      {t("dataMigration.confirmPassword")}
                    </Label>
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

              <Button
                disabled={busy || !Object.values(selection).some(Boolean)}
                onClick={handleExport}
              >
                <Download size={16} />
                {busy ? t("common.loading") : t("dataMigration.exportAction")}
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
        )}

        {activeView === "import" && (
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
                <FileSearch size={16} />
                {t("dataMigration.chooseBackup")}
              </Button>
              {importPath && (
                <p className="text-muted-foreground text-xs break-all">{importPath}</p>
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
                  </div>

                  {inspectResult.requiresPassword && !inspectResult.passwordVerified && (
                    <div className="space-y-2">
                      <Label htmlFor="import-backup-password">
                        {t("dataMigration.backupPassword")}
                      </Label>
                      <div className="flex gap-2">
                        <Input
                          id="import-backup-password"
                          type={showPassword ? "text" : "password"}
                          value={backupPassword}
                          disabled={busy}
                          onChange={(event) => setBackupPassword(event.target.value)}
                        />
                        <Button
                          variant="outline"
                          disabled={busy || !backupPassword}
                          onClick={handleVerifyPassword}
                        >
                          {t("dataMigration.verify")}
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

                  <div className="grid gap-3 sm:grid-cols-2">
                    <div>
                      <Label htmlFor="migration-conflict">
                        {t("dataMigration.conflictStrategy")}
                      </Label>
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
                    {selection.secrets && (
                      <div>
                        <Label htmlFor="migration-secret-destination">
                          {t("dataMigration.secretDestination")}
                        </Label>
                        <Select
                          id="migration-secret-destination"
                          className="mt-1.5"
                          value={secretDestination}
                          disabled={busy}
                          onChange={(event) => setSecretDestination(event.target.value)}
                        >
                          <option value="auto">{t("dataMigration.destinationAuto")}</option>
                          <option value="system">{t("dataMigration.destinationSystem")}</option>
                          <option value="vault">{t("dataMigration.destinationVault")}</option>
                          <option value="hybrid">{t("dataMigration.destinationHybrid")}</option>
                        </Select>
                        {secretDestination === "hybrid" && (
                          <p className="text-muted-foreground mt-1.5 text-xs">
                            {t("dataMigration.hybridPasswordNotice")}
                          </p>
                        )}
                      </div>
                    )}
                  </div>

                  <Button
                    disabled={
                      busy ||
                      !Object.values(selection).some(Boolean) ||
                      (inspectResult.requiresPassword && !inspectResult.passwordVerified)
                    }
                    onClick={handleImport}
                  >
                    <Upload size={16} />
                    {t("dataMigration.importAction")}
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
                    {importResult.requiresRestart && (
                      <Button size="sm" onClick={() => void relaunch()}>
                        <RefreshCw size={14} />
                        {t("dataMigration.restartNow")}
                      </Button>
                    )}
                  </div>
                </Alert>
              )}
            </CardContent>
          </Card>
        )}
      </div>
    </ScrollArea>
  )
}
