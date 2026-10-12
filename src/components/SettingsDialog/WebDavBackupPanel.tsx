import React, { useCallback, useEffect, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { save as saveFileDialog } from "@tauri-apps/plugin-dialog"
import {
  Cloud,
  CloudUpload,
  KeyRound,
  Loader2,
  Lock,
  PlugZap,
  Save,
  Trash2,
  Unlock,
} from "lucide-react"
import { useTranslation } from "react-i18next"

import {
  type WebDavBackupSettings,
  type WebDavBackupStatus,
  type WebDavUploadResult,
  type WebDavDeleteResult,
  type RemoteBackupEntry,
  type BusyAction,
  backupLocation,
} from "@/components/SettingsDialog/webdavBackup"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { useConfig } from "@/contexts/ConfigContext"
import { useUserVerification } from "@/contexts/UserVerificationContext"
import { useToast } from "@/hooks/use-toast"
import { WebDavSyncCard } from "@/components/SettingsDialog/WebDavSyncCard"
import { readBackupFrontendState } from "@/lib/backupFrontendState"
import { isVerificationCanceled } from "@/lib/userVerification"
import { toErrorMessage } from "@/lib/utils"
import { isVaultLocked, vaultUnlockable } from "@/lib/vaultLock"
import {
  withSelection,
  type SelectionItem,
  type SelectionKey,
} from "@/components/SettingsDialog/backupShared"
import { RemoteBackupsCard } from "@/components/SettingsDialog/RemoteBackupsCard"
import { WebDavBackupOptions } from "@/components/SettingsDialog/WebDavBackupOptions"
import { WebDavServerFields } from "@/components/SettingsDialog/WebDavServerFields"

interface WebDavBackupPanelProps {
  selectionItems: SelectionItem[]
  /** Opens a downloaded backup in the import view. */
  onRestore: (localPath: string, entry: RemoteBackupEntry) => Promise<void>
}
export const WebDavBackupPanel: React.FC<WebDavBackupPanelProps> = ({
  selectionItems,
  onRestore,
}) => {
  const { t } = useTranslation()
  const { toast } = useToast()
  const { withVerification, withVaultUnlock, requestVaultUnlock } = useUserVerification()
  const { isSecretStatusLoaded, secretStatus } = useConfig()
  const [status, setStatus] = useState<WebDavBackupStatus | null>(null)
  const [form, setForm] = useState<WebDavBackupSettings | null>(null)
  const [password, setPassword] = useState("")
  const [backupPassword, setBackupPassword] = useState("")
  const [confirmBackupPassword, setConfirmBackupPassword] = useState("")
  const [showPasswords, setShowPasswords] = useState(false)
  const [busyAction, setBusyAction] = useState<BusyAction | null>(null)
  const busy = busyAction !== null
  const [remoteBackups, setRemoteBackups] = useState<RemoteBackupEntry[] | null>(null)
  const [listError, setListError] = useState<string | null>(null)
  // The remote file being restored or saved, for its row's spinner.
  const [rowAction, setRowAction] = useState<{
    fileName: string
    kind: "restore" | "download"
  } | null>(null)
  const [selected, setSelected] = useState<Set<string>>(() => new Set())

  const configured = Boolean(status?.settings.url && status.hasPassword)
  // The WebDAV passwords are kept with the other saved passwords.
  const vaultLocked =
    isSecretStatusLoaded && !secretStatus.unlocked && secretStatus.storageMode !== "memory"
  const unlockable = vaultUnlockable(secretStatus)

  const applyStatus = useCallback((next: WebDavBackupStatus) => {
    setStatus(next)
    setForm(next.settings)
  }, [])

  /** `prompt` asks to unlock saved passwords when they are locked. */
  const loadRemoteBackups = useCallback(
    async (prompt = true) => {
      setBusyAction("list")
      try {
        const list = () => invoke<RemoteBackupEntry[]>("list_webdav_backups")
        const backups = await (prompt ? withVaultUnlock(list) : list())
        setRemoteBackups(backups)
        // Files gone from the server drop out of the selection.
        const names = new Set(backups.map((entry) => entry.fileName))
        setSelected((current) => new Set([...current].filter((name) => names.has(name))))
        setListError(null)
      } catch (error) {
        // While locked, the list shows the unlock notice instead.
        if (isVerificationCanceled(error) || (!prompt && isVaultLocked(error))) return
        setListError(toErrorMessage(error))
      } finally {
        setBusyAction(null)
      }
    },
    [withVaultUnlock]
  )

  useEffect(() => {
    invoke<WebDavBackupStatus>("get_webdav_backup_status")
      .then((next) => {
        applyStatus(next)
        // Opening the panel does not ask for the master password.
        if (next.settings.url && next.hasPassword) void loadRemoteBackups(false)
      })
      .catch((error) => console.error("Failed to load WebDAV backup settings:", error))
  }, [applyStatus, loadRemoteBackups])

  if (!status || !form) return null

  const update = (patch: Partial<WebDavBackupSettings>) => setForm({ ...form, ...patch })
  const location = backupLocation(form.url, form.remoteDirectory)

  const fail = (title: string, error: unknown) => {
    if (isVerificationCanceled(error)) return
    toast({ title, description: toErrorMessage(error), variant: "destructive" })
  }

  /** Saves the form; returns false (after telling the user) when it could not. */
  const save = async () => {
    if (backupPassword !== confirmBackupPassword) {
      toast({ title: t("dataMigration.passwordMismatch"), variant: "destructive" })
      return false
    }
    try {
      // Another server, account or backup password asks for verification.
      applyStatus(
        await withVerification("sensitive", () =>
          invoke<WebDavBackupStatus>("save_webdav_backup_settings", {
            settings: form,
            password: password || null,
            backupPassword: backupPassword || null,
          })
        )
      )
      setPassword("")
      setBackupPassword("")
      setConfirmBackupPassword("")
      return true
    } catch (error) {
      fail(t("dataMigration.webdav.saveFailed"), error)
      return false
    }
  }

  const handleSave = async () => {
    setBusyAction("save")
    try {
      if (await save()) {
        toast({ title: t("dataMigration.webdav.saved") })
        void loadRemoteBackups()
      }
    } finally {
      setBusyAction(null)
    }
  }

  const handleUnlock = async () => {
    if ((await requestVaultUnlock()) && configured) void loadRemoteBackups()
  }

  const handleTest = async () => {
    setBusyAction("test")
    try {
      await withVaultUnlock(() =>
        invoke("test_webdav_connection", { settings: form, password: password || null })
      )
      toast({ title: t("dataMigration.webdav.testSuccess") })
    } catch (error) {
      fail(t("dataMigration.webdav.testFailed"), error)
    } finally {
      setBusyAction(null)
    }
  }

  const handleUpload = async () => {
    setBusyAction("upload")
    // A form that cannot be saved keeps the user's edits.
    if (!(await save())) {
      setBusyAction(null)
      return
    }
    try {
      const result = await withVaultUnlock(() =>
        invoke<WebDavUploadResult | null>("run_webdav_backup", {
          frontendState: readBackupFrontendState(),
          force: true,
        })
      )
      if (result) {
        toast({
          title: t("dataMigration.webdav.uploadSuccess"),
          description: t("dataMigration.exportSummary", {
            profiles: result.profileCount,
            commands: result.commandCount,
            secrets: result.secretCount,
          }),
        })
        if (result.pruneError) {
          fail(t("dataMigration.webdav.pruneFailed"), result.pruneError)
        }
      }
    } catch (error) {
      fail(t("dataMigration.webdav.uploadFailed"), error)
    } finally {
      // The upload records its time or error in the settings.
      applyStatus(await invoke<WebDavBackupStatus>("get_webdav_backup_status").catch(() => status))
      setBusyAction(null)
    }
    void loadRemoteBackups()
  }

  const handleRestore = async (entry: RemoteBackupEntry) => {
    setBusyAction("download")
    setRowAction({ fileName: entry.fileName, kind: "restore" })
    try {
      const localPath = await withVaultUnlock(() =>
        invoke<string>("download_webdav_backup", { fileName: entry.fileName })
      )
      toast({
        title: t("dataMigration.webdav.restoreDownloaded", { name: entry.fileName }),
        description: t("dataMigration.webdav.restoreInspecting"),
      })
      await onRestore(localPath, entry)
    } catch (error) {
      fail(t("dataMigration.webdav.downloadFailed"), error)
    } finally {
      setRowAction(null)
      setBusyAction(null)
    }
  }

  const handleDownload = async (entry: RemoteBackupEntry) => {
    const outputPath = await saveFileDialog({
      defaultPath: entry.fileName,
      filters: [{ name: "tTerm Backup", extensions: ["tterm-backup"] }],
    })
    if (!outputPath) return
    setBusyAction("download")
    setRowAction({ fileName: entry.fileName, kind: "download" })
    try {
      const savedPath = await withVaultUnlock(() =>
        invoke<string>("save_webdav_backup", { fileName: entry.fileName, outputPath })
      )
      toast({ title: t("dataMigration.webdav.downloadSuccess"), description: savedPath })
    } catch (error) {
      fail(t("dataMigration.webdav.downloadFailed"), error)
    } finally {
      setRowAction(null)
      setBusyAction(null)
    }
  }

  const deleteBackups = async (fileNames: string[]) => {
    setBusyAction("delete")
    try {
      const result = await withVaultUnlock(() =>
        invoke<WebDavDeleteResult>("delete_webdav_backups", { fileNames })
      )
      const deleted = new Set(result.deleted)
      setRemoteBackups((current) =>
        current ? current.filter((item) => !deleted.has(item.fileName)) : current
      )
      setSelected((current) => new Set([...current].filter((name) => !deleted.has(name))))
      if (result.error) fail(t("dataMigration.deleteBackupFailed"), result.error)
    } catch (error) {
      fail(t("dataMigration.deleteBackupFailed"), error)
    } finally {
      setBusyAction(null)
    }
  }

  const handleDelete = (entry: RemoteBackupEntry) => {
    if (!window.confirm(t("dataMigration.deleteBackupConfirm", { name: entry.fileName }))) return
    void deleteBackups([entry.fileName])
  }

  const handleDeleteSelected = () => {
    if (!window.confirm(t("dataMigration.webdav.deleteSelectedConfirm", { count: selected.size })))
      return
    void deleteBackups([...selected])
  }

  const toggleSelected = (fileName: string, checked: boolean) =>
    setSelected((current) => {
      const next = new Set(current)
      if (checked) next.add(fileName)
      else next.delete(fileName)
      return next
    })

  const allSelected = Boolean(remoteBackups?.length) && selected.size === remoteBackups?.length

  const handleClear = async () => {
    if (!window.confirm(t("dataMigration.webdav.clearConfirm"))) return
    setBusyAction("clear")
    try {
      applyStatus(await invoke<WebDavBackupStatus>("clear_webdav_backup_settings"))
      setRemoteBackups(null)
      setSelected(new Set())
      setListError(null)
      setPassword("")
      setBackupPassword("")
      setConfirmBackupPassword("")
    } catch (error) {
      fail(t("dataMigration.webdav.clearFailed"), error)
    } finally {
      setBusyAction(null)
    }
  }

  const updateSelection = (key: SelectionKey, checked: boolean) =>
    update({ selection: withSelection(form.selection, key, checked) })

  const savedPlaceholder = t("dataMigration.webdav.passwordSaved")
  const passwordType = showPasswords ? "text" : "password"

  return (
    <>
      <Card>
        <CardContent className="space-y-4 p-4">
          <div>
            <div className="flex items-center gap-2 text-sm font-medium">
              <Cloud size={16} />
              {t("dataMigration.webdav.title")}
            </div>
            <p className="text-muted-foreground mt-1 text-xs">
              {t("dataMigration.webdav.description")}
            </p>
          </div>

          {vaultLocked && (
            <Alert className="border-amber-500/40 bg-amber-500/10">
              <Lock size={16} className="absolute top-3.5 left-4" />
              <div className="flex flex-wrap items-center justify-between gap-3 pl-6">
                <div className="min-w-0 flex-1">
                  <AlertTitle>{t("dataMigration.webdav.lockedTitle")}</AlertTitle>
                  <AlertDescription>
                    {unlockable
                      ? t("dataMigration.webdav.lockedDescription")
                      : t("dataMigration.webdav.lockedSettingsHint")}
                  </AlertDescription>
                </div>
                {unlockable && (
                  <Button
                    type="button"
                    size="sm"
                    variant="outline"
                    onClick={() => void handleUnlock()}
                  >
                    <Unlock size={16} />
                    {t("secretStorage.unlock")}
                  </Button>
                )}
              </div>
            </Alert>
          )}

          <WebDavServerFields
            form={form}
            status={status}
            busy={busy}
            update={update}
            password={password}
            setPassword={setPassword}
            passwordType={passwordType}
            savedPlaceholder={savedPlaceholder}
            showPasswords={showPasswords}
            setShowPasswords={setShowPasswords}
            location={location}
          />

          <Alert className="border-amber-500/40 bg-amber-500/10">
            <KeyRound size={16} className="absolute top-3.5 left-4" />
            <div className="pl-6">
              <AlertTitle>{t("dataMigration.webdav.encryptionTitle")}</AlertTitle>
              <AlertDescription>{t("dataMigration.webdav.encryptionDescription")}</AlertDescription>
            </div>
          </Alert>

          <WebDavBackupOptions
            form={form}
            status={status}
            busy={busy}
            update={update}
            backupPassword={backupPassword}
            confirmBackupPassword={confirmBackupPassword}
            setBackupPassword={setBackupPassword}
            setConfirmBackupPassword={setConfirmBackupPassword}
            passwordType={passwordType}
            savedPlaceholder={savedPlaceholder}
            updateSelection={updateSelection}
            selectionItems={selectionItems}
          />

          <div className="flex flex-wrap gap-2">
            <Button disabled={busy} onClick={handleSave}>
              {busyAction === "save" ? (
                <Loader2 className="animate-spin" size={16} />
              ) : (
                <Save size={16} />
              )}
              {t("common.save")}
            </Button>
            <Button variant="outline" disabled={busy || !form.url.trim()} onClick={handleTest}>
              {busyAction === "test" ? (
                <Loader2 className="animate-spin" size={16} />
              ) : (
                <PlugZap size={16} />
              )}
              {busyAction === "test"
                ? t("dataMigration.webdav.testing")
                : t("dataMigration.webdav.test")}
            </Button>
            <Button variant="outline" disabled={busy || !form.url.trim()} onClick={handleUpload}>
              {busyAction === "upload" ? (
                <Loader2 className="animate-spin" size={16} />
              ) : (
                <CloudUpload size={16} />
              )}
              {busyAction === "upload"
                ? t("dataMigration.webdav.uploading")
                : t("dataMigration.webdav.upload")}
            </Button>
            {configured && (
              <Button
                variant="ghost"
                className="text-destructive ml-auto"
                disabled={busy}
                onClick={handleClear}
              >
                <Trash2 size={16} />
                {t("dataMigration.webdav.clear")}
              </Button>
            )}
          </div>
        </CardContent>
      </Card>

      {configured && <WebDavSyncCard />}

      <RemoteBackupsCard
        busy={busy}
        busyAction={busyAction}
        remoteBackups={remoteBackups}
        listError={listError}
        rowAction={rowAction}
        selected={selected}
        setSelected={setSelected}
        configured={configured}
        vaultLocked={vaultLocked}
        allSelected={allSelected}
        loadRemoteBackups={loadRemoteBackups}
        handleRestore={handleRestore}
        handleDownload={handleDownload}
        handleDelete={handleDelete}
        handleDeleteSelected={handleDeleteSelected}
        toggleSelected={toggleSelected}
      />
    </>
  )
}
