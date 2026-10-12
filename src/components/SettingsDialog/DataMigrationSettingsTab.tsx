import React, { useCallback, useEffect, useMemo, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { open as openFileDialog, save as saveFileDialog } from "@tauri-apps/plugin-dialog"
import { Archive } from "lucide-react"
import { useTranslation } from "react-i18next"

import {
  type BackupInspectResult,
  type AutomaticBackupSettings,
  type BackupHistoryEntry,
  type BackupExportResult,
  type BackupImportResult,
  type MigrationView,
  defaultSelection,
  cloneAvailableSelection,
  restoreFrontendState,
  type BusyAction,
} from "@/components/SettingsDialog/dataMigration"
import { ScrollArea } from "@/components/ui/scroll-area"
import { useTheme } from "@/contexts/ThemeContext"
import { useUserVerification } from "@/contexts/UserVerificationContext"
import { useToast } from "@/hooks/use-toast"
import { readBackupFrontendState } from "@/lib/backupFrontendState"
import { isVerificationCanceled } from "@/lib/userVerification"
import { toErrorMessage } from "@/lib/utils"
import type { RemoteBackupEntry } from "@/components/SettingsDialog/webdavBackup"
import { WebDavBackupPanel } from "@/components/SettingsDialog/WebDavBackupPanel"
import {
  withSelection,
  type BackupSelection,
  type SelectionKey,
} from "@/components/SettingsDialog/backupShared"
import { AutomaticBackupCard } from "@/components/SettingsDialog/AutomaticBackupCard"
import { BackupExportCard } from "@/components/SettingsDialog/BackupExportCard"
import { BackupHistoryCard } from "@/components/SettingsDialog/BackupHistoryCard"
import { BackupImportCard } from "@/components/SettingsDialog/BackupImportCard"
import { MigrationViewTabs } from "@/components/SettingsDialog/MigrationViewTabs"

export const DataMigrationSettingsTab: React.FC = () => {
  const { t } = useTranslation()
  const { toast } = useToast()
  const { withVerification } = useUserVerification()
  const { reloadCustomThemes } = useTheme()
  const [selection, setSelection] = useState<BackupSelection>(defaultSelection)
  const [backupPassword, setBackupPassword] = useState("")
  const [importPassword, setImportPassword] = useState("")
  const [confirmPassword, setConfirmPassword] = useState("")
  const [showPassword, setShowPassword] = useState(false)
  // Which long-running action is in progress; its button shows a spinner.
  const [busyAction, setBusyAction] = useState<BusyAction | null>(null)
  const busy = busyAction !== null
  const [activeView, setActiveView] = useState<MigrationView>("backup")
  const [importPath, setImportPath] = useState("")
  // The WebDAV backup the import file was downloaded from.
  const [importSource, setImportSource] = useState<RemoteBackupEntry | null>(null)
  const [inspectResult, setInspectResult] = useState<BackupInspectResult | null>(null)
  const [conflictStrategy, setConflictStrategy] = useState("merge")
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

  // The state is set in the promise callbacks rather than after an `await`,
  // which the effect lint can't yet tell apart from setting it synchronously.
  const refreshBackupManagement = useCallback(
    () =>
      Promise.all([
        invoke<AutomaticBackupSettings>("get_automatic_backup_settings"),
        invoke<BackupHistoryEntry[]>("list_backup_history"),
      ]).then(([settings, history]) => {
        setAutomaticSettings(settings)
        setBackupHistory(history)
      }),
    []
  )

  useEffect(() => {
    refreshBackupManagement().catch((error) => {
      console.error("Failed to load backup management settings:", error)
    })
  }, [refreshBackupManagement])

  // A backup downloaded from WebDAV lives only while the import view shows it.
  const fromWebDav = importSource !== null
  useEffect(() => {
    if (!fromWebDav) return
    return () => {
      invoke("discard_webdav_download").catch((error) => {
        console.error("Failed to delete the downloaded WebDAV backup:", error)
      })
    }
  }, [fromWebDav])

  const showView = (view: MigrationView) => {
    if (view !== "import" && importSource) {
      setImportSource(null)
      setImportPath("")
      setImportPassword("")
      setInspectResult(null)
      setImportResult(null)
    }
    setActiveView(view)
  }

  const updateSelection = (key: SelectionKey, checked: boolean) => {
    setSelection((current) => withSelection(current, key, checked))
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

    setBusyAction("export")
    setExportResult(null)
    try {
      const result = await withVerification("sensitive", () =>
        invoke<BackupExportResult>("export_backup", {
          outputPath,
          options: {
            selection,
            backupPassword: backupPassword || null,
            frontendState: readBackupFrontendState(),
          },
        })
      )
      setExportResult(result)
      setBackupPassword("")
      setConfirmPassword("")
      toast({ title: t("dataMigration.exportSuccess") })
    } catch (error) {
      if (isVerificationCanceled(error)) return
      toast({
        title: t("dataMigration.exportFailed"),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    } finally {
      setBusyAction(null)
    }
  }

  const inspect = async (path: string, password: string, fromWebDav = importSource !== null) => {
    const result = await invoke<BackupInspectResult>("inspect_backup", {
      input: {
        inputPath: path,
        backupPassword: password || null,
        useWebdavPassword: fromWebDav,
      },
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
    setBusyAction("inspect")
    setImportPath(selected)
    setImportSource(null)
    setImportPassword("")
    setImportResult(null)
    try {
      await inspect(selected, "", false)
    } catch (error) {
      setInspectResult(null)
      toast({
        title: t("dataMigration.inspectFailed"),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    } finally {
      setBusyAction(null)
    }
  }

  const handleVerifyPassword = async () => {
    if (!importPath) return
    setBusyAction("verify")
    try {
      await inspect(importPath, importPassword)
      toast({ title: t("dataMigration.passwordVerified") })
    } catch (error) {
      toast({
        title: t("dataMigration.passwordInvalid"),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    } finally {
      setBusyAction(null)
    }
  }

  const handleImport = async () => {
    if (!importPath || !inspectResult || !Object.values(selection).some(Boolean)) return
    if (inspectResult.requiresPassword && !inspectResult.passwordVerified) {
      await handleVerifyPassword()
      return
    }
    if (!window.confirm(t("dataMigration.importConfirm"))) return

    setBusyAction("import")
    try {
      // Taking another WebDAV server or backup password from the file asks
      // for verification first.
      const result = await withVerification("sensitive", () =>
        invoke<BackupImportResult>("import_backup", {
          inputPath: importPath,
          options: {
            selection,
            backupPassword: importPassword || null,
            useWebdavPassword: importSource !== null,
            conflictStrategy,
          },
        })
      )
      restoreFrontendState(result.frontendState)
      if (selection.themes) void reloadCustomThemes()
      setImportResult(result)
      // The backup password stays until another file is chosen, so the same
      // backup can be imported again (e.g. merge, then replace) without
      // re-entering it; the password field is hidden once it is verified.
      toast({ title: t("dataMigration.importSuccess") })
      try {
        // Refresh the diff against the data as it is now; keep the selection.
        setInspectResult(
          await invoke<BackupInspectResult>("inspect_backup", {
            input: {
              inputPath: importPath,
              backupPassword: importPassword || null,
              useWebdavPassword: importSource !== null,
            },
          })
        )
        // The import may have brought its own backup schedule.
        await refreshBackupManagement()
      } catch {
        // The import succeeded; a stale diff is not worth an error.
      }
    } catch (error) {
      if (isVerificationCanceled(error)) return
      toast({
        title: t("dataMigration.importFailed"),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    } finally {
      setBusyAction(null)
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
    setBusyAction("saveSettings")
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
      setBusyAction(null)
    }
  }

  const handleBackupNow = async () => {
    if (!automaticSettings) return
    setBusyAction("backupNow")
    try {
      await invoke("save_automatic_backup_settings", { settings: automaticSettings })
      await invoke("run_due_automatic_backup", {
        frontendState: readBackupFrontendState(),
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
      setBusyAction(null)
    }
  }

  const handleDeleteHistory = async (entry: BackupHistoryEntry) => {
    if (!window.confirm(t("dataMigration.deleteBackupConfirm", { name: entry.fileName }))) return
    setBusyAction("deleteHistory")
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
      setBusyAction(null)
    }
  }

  const openForImport = async (path: string, source: RemoteBackupEntry | null = null) => {
    setBusyAction("inspect")
    setActiveView("import")
    setImportPath(path)
    setImportSource(source)
    setImportPassword("")
    setImportResult(null)
    setInspectResult(null)
    try {
      await inspect(path, "", source !== null)
    } catch (error) {
      toast({
        title: t("dataMigration.inspectFailed"),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    } finally {
      setBusyAction(null)
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

        <MigrationViewTabs activeView={activeView} showView={showView} />

        {activeView === "backup" && automaticSettings && (
          <AutomaticBackupCard
            busyAction={busyAction}
            busy={busy}
            automaticSettings={automaticSettings}
            setAutomaticSettings={setAutomaticSettings}
            selectionItems={selectionItems}
            updateAutomaticSelection={updateAutomaticSelection}
            handleChooseAutomaticDirectory={handleChooseAutomaticDirectory}
            handleSaveAutomaticSettings={handleSaveAutomaticSettings}
            handleBackupNow={handleBackupNow}
          />
        )}

        {activeView === "webdav" && (
          <WebDavBackupPanel selectionItems={selectionItems} onRestore={openForImport} />
        )}

        {activeView === "history" && (
          <BackupHistoryCard
            busy={busy}
            backupHistory={backupHistory}
            handleDeleteHistory={handleDeleteHistory}
            openForImport={openForImport}
          />
        )}

        {activeView === "backup" && (
          <BackupExportCard
            selection={selection}
            backupPassword={backupPassword}
            confirmPassword={confirmPassword}
            setBackupPassword={setBackupPassword}
            setConfirmPassword={setConfirmPassword}
            showPassword={showPassword}
            setShowPassword={setShowPassword}
            busyAction={busyAction}
            busy={busy}
            exportResult={exportResult}
            selectionItems={selectionItems}
            updateSelection={updateSelection}
            handleExport={handleExport}
          />
        )}

        {activeView === "import" && (
          <BackupImportCard
            selection={selection}
            importPassword={importPassword}
            setImportPassword={setImportPassword}
            showPassword={showPassword}
            busyAction={busyAction}
            busy={busy}
            importPath={importPath}
            importSource={importSource}
            inspectResult={inspectResult}
            conflictStrategy={conflictStrategy}
            setConflictStrategy={setConflictStrategy}
            importResult={importResult}
            selectionItems={selectionItems}
            showView={showView}
            updateSelection={updateSelection}
            handleChooseImport={handleChooseImport}
            handleVerifyPassword={handleVerifyPassword}
            handleImport={handleImport}
          />
        )}
      </div>
    </ScrollArea>
  )
}
