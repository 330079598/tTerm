import React, { useCallback, useEffect, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { save as saveFileDialog } from "@tauri-apps/plugin-dialog"
import {
  AlertTriangle,
  CheckCircle2,
  Cloud,
  CloudUpload,
  Download,
  Eye,
  EyeOff,
  KeyRound,
  Loader2,
  Lock,
  PlugZap,
  RefreshCw,
  RotateCcw,
  Save,
  Trash2,
  Unlock,
} from "lucide-react"
import { useTranslation } from "react-i18next"

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import { useConfig } from "@/contexts/ConfigContext"
import { useUserVerification } from "@/contexts/UserVerificationContext"
import { useToast } from "@/hooks/use-toast"
import { WebDavSyncCard } from "@/components/SettingsDialog/WebDavSyncCard"
import { readBackupFrontendState } from "@/lib/backupFrontendState"
import { isVerificationCanceled } from "@/lib/userVerification"
import { toErrorMessage } from "@/lib/utils"
import { isVaultLocked, vaultUnlockable } from "@/lib/vaultLock"
import {
  formatFileSize,
  withSelection,
  type BackupSelection,
  type SelectionItem,
  type SelectionKey,
} from "@/components/SettingsDialog/backupShared"

interface WebDavBackupSettings {
  url: string
  username: string
  remoteDirectory: string
  frequency: "off" | "daily" | "weekly"
  retentionCount: number
  selection: BackupSelection
  /** Upload history, sent by the backend and ignored when saved. */
  lastBackupAt?: number | null
  lastError?: string | null
}

interface WebDavBackupStatus {
  settings: WebDavBackupSettings
  hasPassword: boolean
  hasBackupPassword: boolean
  deviceName: string
}

interface WebDavUploadResult {
  fileName: string
  sizeBytes: number
  profileCount: number
  commandCount: number
  secretCount: number
  pruneError: string | null
}

interface WebDavDeleteResult {
  deleted: string[]
  error: string | null
}

export interface RemoteBackupEntry {
  fileName: string
  sizeBytes: number
  createdAt: number | null
  device: string | null
  currentDevice: boolean
}

type BusyAction = "save" | "test" | "upload" | "list" | "download" | "delete" | "clear"

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

          <div>
            <Label htmlFor="webdav-url">{t("dataMigration.webdav.url")}</Label>
            <Input
              id="webdav-url"
              className="mt-1.5"
              value={form.url}
              disabled={busy}
              placeholder="https://dav.jianguoyun.com/dav/"
              spellCheck={false}
              autoCapitalize="off"
              onChange={(event) => update({ url: event.target.value })}
            />
            <p className="text-muted-foreground mt-1 text-xs">
              {t("dataMigration.webdav.urlHint")}
            </p>
          </div>

          {form.url.trim().toLowerCase().startsWith("http://") && (
            <Alert className="border-amber-500/40 bg-amber-500/10">
              <AlertTriangle size={16} className="absolute top-3.5 left-4" />
              <AlertDescription className="pl-6">
                {t("dataMigration.webdav.insecureUrl")}
              </AlertDescription>
            </Alert>
          )}

          <div className="grid gap-3 sm:grid-cols-2">
            <div>
              <Label htmlFor="webdav-username">{t("dataMigration.webdav.username")}</Label>
              <Input
                id="webdav-username"
                className="mt-1.5"
                value={form.username}
                disabled={busy}
                spellCheck={false}
                autoCapitalize="off"
                onChange={(event) => update({ username: event.target.value })}
              />
            </div>
            <div>
              <Label htmlFor="webdav-password">{t("dataMigration.webdav.password")}</Label>
              <div className="relative mt-1.5">
                <Input
                  id="webdav-password"
                  type={passwordType}
                  value={password}
                  disabled={busy}
                  placeholder={status.hasPassword ? savedPlaceholder : ""}
                  onChange={(event) => setPassword(event.target.value)}
                  className="pr-10"
                />
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  className="absolute top-0 right-0"
                  onClick={() => setShowPasswords((visible) => !visible)}
                  aria-label={t("dataMigration.togglePassword")}
                >
                  {showPasswords ? <EyeOff size={16} /> : <Eye size={16} />}
                </Button>
              </div>
            </div>
          </div>

          <div>
            <Label htmlFor="webdav-directory">{t("dataMigration.webdav.remoteDirectory")}</Label>
            <Input
              id="webdav-directory"
              className="mt-1.5"
              value={form.remoteDirectory}
              disabled={busy}
              spellCheck={false}
              onChange={(event) => update({ remoteDirectory: event.target.value })}
            />
          </div>

          <Alert className="border-amber-500/40 bg-amber-500/10">
            <KeyRound size={16} className="absolute top-3.5 left-4" />
            <div className="pl-6">
              <AlertTitle>{t("dataMigration.webdav.encryptionTitle")}</AlertTitle>
              <AlertDescription>{t("dataMigration.webdav.encryptionDescription")}</AlertDescription>
            </div>
          </Alert>

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
                <AlertDescription className="break-all">
                  {status.settings.lastError}
                </AlertDescription>
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
            <p className="text-muted-foreground text-sm">
              {t("dataMigration.webdav.notConfigured")}
            </p>
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
                            <Badge variant="secondary">
                              {t("dataMigration.webdav.thisDevice")}
                            </Badge>
                          ) : (
                            entry.device && <Badge variant="outline">{entry.device}</Badge>
                          )}
                        </div>
                        <div className="text-muted-foreground mt-1 text-xs">
                          {entry.createdAt
                            ? `${new Date(entry.createdAt).toLocaleString()} · `
                            : ""}
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
    </>
  )
}
