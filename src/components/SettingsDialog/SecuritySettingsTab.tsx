import React, { useCallback, useEffect, useRef, useState } from "react"
import {
  AlertTriangle,
  Eye,
  EyeOff,
  KeyRound,
  Loader2,
  Lock,
  Shield,
  Trash2,
  Unlock,
} from "lucide-react"
import { useTranslation } from "react-i18next"

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Select } from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"

import { SettingsRow, SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import type { VaultAction } from "@/components/SettingsDialog/types"
import type { useConfirmDialog, useInfoDialog } from "@/components/ui/app-dialog"
import { useConfig, type SavedSecretEntry, type SecretStorageMode } from "@/contexts/ConfigContext"
import { useToast } from "@/hooks/use-toast"
import { toErrorMessage } from "@/lib/utils"

interface SecuritySettingsTabProps {
  confirm: ReturnType<typeof useConfirmDialog>["confirm"]
  info: ReturnType<typeof useInfoDialog>["info"]
}

interface PasswordFieldProps {
  id: string
  label: string
  value: string
  onChange: (value: string) => void
  disabled: boolean
  autoComplete: "current-password" | "new-password"
  autoFocus?: boolean
}

const PasswordField: React.FC<PasswordFieldProps> = ({
  id,
  label,
  value,
  onChange,
  disabled,
  autoComplete,
  autoFocus,
}) => {
  const { t } = useTranslation()
  const [visible, setVisible] = useState(false)
  const toggleLabel = t(visible ? "secretStorage.hidePassword" : "secretStorage.showPassword")
  return (
    <div>
      <Label htmlFor={id} className="mb-1.5 block">
        {label}
      </Label>
      <div className="relative">
        <Input
          id={id}
          type={visible ? "text" : "password"}
          autoComplete={autoComplete}
          autoFocus={autoFocus}
          value={value}
          onChange={(event) => onChange(event.target.value)}
          placeholder={t("secretStorage.masterPasswordPlaceholder")}
          disabled={disabled}
          className="pr-9"
        />
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          onClick={() => setVisible((current) => !current)}
          className="text-muted-foreground hover:text-foreground absolute top-1/2 right-1 -translate-y-1/2"
          aria-label={toggleLabel}
          title={toggleLabel}
        >
          {visible ? <EyeOff size={16} /> : <Eye size={16} />}
        </Button>
      </div>
    </div>
  )
}

const ActionIcon: React.FC<{ busy: boolean; icon: React.ReactNode }> = ({ busy, icon }) =>
  busy ? <Loader2 size={14} className="mr-2 animate-spin" /> : <>{icon}</>

export const SecuritySettingsTab: React.FC<SecuritySettingsTabProps> = ({ confirm, info }) => {
  const { t } = useTranslation()
  const { toast } = useToast()
  const {
    config,
    saveConfig,
    secretStatus,
    setSecretStorageMode,
    unlockSecretVault,
    lockSecretVault,
    changeVaultPassword,
    setMasterPassword,
    removeMasterPassword,
    listSavedSecrets,
    getSavedSecret,
    deleteSavedSecret,
  } = useConfig()

  const [password, setPassword] = useState("")
  const [currentPassword, setCurrentPassword] = useState("")
  const [newPassword, setNewPassword] = useState("")
  const [confirmPassword, setConfirmPassword] = useState("")
  /** Master password mode chosen but waiting for a password. */
  const [choosingPasswordMode, setChoosingPasswordMode] = useState(false)
  const [action, setAction] = useState<VaultAction | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [savedSecrets, setSavedSecrets] = useState<SavedSecretEntry[]>([])
  const [revealedSecretKey, setRevealedSecretKey] = useState<string | null>(null)
  const [revealedPassword, setRevealedPassword] = useState<string | null>(null)
  const [revealError, setRevealError] = useState(false)
  const revealTimerRef = useRef<number | null>(null)
  const revealRequestRef = useRef(0)
  const isMountedRef = useRef(true)

  const busy = action !== null
  const mode = secretStatus.storageMode
  const pending = secretStatus.migrationPending
  const { unlocked, hasMasterPassword } = secretStatus

  const reloadSavedSecrets = useCallback(async () => {
    try {
      const entries = await listSavedSecrets()
      if (isMountedRef.current) setSavedSecrets(entries)
    } catch {
      if (isMountedRef.current) setSavedSecrets([])
    }
  }, [listSavedSecrets])

  const clearRevealedPassword = useCallback(() => {
    if (revealTimerRef.current !== null) {
      window.clearTimeout(revealTimerRef.current)
      revealTimerRef.current = null
    }
    setRevealedSecretKey(null)
    setRevealedPassword(null)
  }, [])

  useEffect(() => {
    isMountedRef.current = true
    void reloadSavedSecrets()
    return () => {
      isMountedRef.current = false
      if (revealTimerRef.current !== null) {
        window.clearTimeout(revealTimerRef.current)
      }
    }
  }, [reloadSavedSecrets])

  const clearPasswordFields = () => {
    setPassword("")
    setCurrentPassword("")
    setNewPassword("")
    setConfirmPassword("")
  }

  /** Runs a secret store action with a spinner on its button. */
  const run = async (name: VaultAction, work: () => Promise<void>) => {
    setAction(name)
    setError(null)
    try {
      await work()
    } catch (caught) {
      if (isMountedRef.current) setError(toErrorMessage(caught))
    } finally {
      if (isMountedRef.current) setAction(null)
    }
  }

  const newPasswordsMatch = async () => {
    if (newPassword === confirmPassword) return true
    await info({
      title: t("secretStorage.masterPassword"),
      description: t("secretStorage.passwordMismatch"),
    })
    return false
  }

  const handleModeChange = async (next: SecretStorageMode) => {
    if (next === "password" && !hasMasterPassword) {
      setChoosingPasswordMode(true)
      clearPasswordFields()
      return
    }
    setChoosingPasswordMode(false)
    await run("mode", async () => {
      await setSecretStorageMode(next)
      toast({
        title: t("secretStorage.modeSaved"),
        description: t(`secretStorage.modeDescriptions.${next}`),
      })
      await reloadSavedSecrets()
    })
  }

  const handleConfirmPasswordMode = async () => {
    if (!(await newPasswordsMatch())) return
    await run("mode", async () => {
      await setSecretStorageMode("password", newPassword)
      clearPasswordFields()
      setChoosingPasswordMode(false)
      toast({
        title: t("secretStorage.modeSaved"),
        description: t("secretStorage.modeDescriptions.password"),
        variant: "success",
      })
    })
  }

  const handleUnlock = async () => {
    const wasPending = pending
    // In master password mode without one yet, unlocking sets it.
    const settingFirstPassword = !wasPending && mode === "password" && !hasMasterPassword
    if (settingFirstPassword && !(await newPasswordsMatch())) return
    await run("unlock", async () => {
      await unlockSecretVault(settingFirstPassword ? newPassword : password)
      clearPasswordFields()
      toast({
        title: wasPending
          ? t("secretStorage.migrated")
          : settingFirstPassword
            ? t("secretStorage.passwordSet")
            : t("secretStorage.unlocked"),
        description: wasPending
          ? t("secretStorage.migratedDesc")
          : settingFirstPassword
            ? t("secretStorage.passwordSetDesc")
            : t("secretStorage.unlockedDesc"),
        variant: "success",
      })
      await reloadSavedSecrets()
    })
  }

  const handleLock = async () => {
    clearRevealedPassword()
    await run("lock", async () => {
      await lockSecretVault()
      toast({ title: t("secretStorage.locked"), description: t("secretStorage.lockedDesc") })
    })
  }

  const handleChangePassword = async () => {
    if (!(await newPasswordsMatch())) return
    await run("changePassword", async () => {
      await changeVaultPassword(currentPassword, newPassword)
      clearPasswordFields()
      toast({
        title: t("secretStorage.passwordChanged"),
        description: t("secretStorage.passwordChangedDesc"),
        variant: "success",
      })
    })
  }

  const handleSetRecovery = async () => {
    if (!(await newPasswordsMatch())) return
    await run("setPassword", async () => {
      await setMasterPassword(newPassword)
      clearPasswordFields()
      toast({
        title: t("secretStorage.recoverySaved"),
        description: t("secretStorage.recoverySavedDesc"),
        variant: "success",
      })
    })
  }

  const handleRemoveRecovery = async () => {
    const confirmed = await confirm({
      title: t("secretStorage.removeRecovery"),
      description: t("secretStorage.removeRecoveryConfirm"),
      confirmText: t("secretStorage.removeRecovery"),
      cancelText: t("common.cancel"),
      variant: "destructive",
    })
    if (!confirmed) return
    await run("removePassword", async () => {
      await removeMasterPassword()
      toast({ title: t("secretStorage.recoveryRemoved") })
    })
  }

  const handlePromptUnlockOnStartupChange = async (checked: boolean) => {
    await run("mode", async () => {
      await saveConfig({ prompt_unlock_vault_on_startup: checked })
      toast({
        title: t("secretStorage.startupPromptSaved"),
        description: checked
          ? t("secretStorage.startupPromptEnabledDesc")
          : t("secretStorage.startupPromptDisabledDesc"),
      })
    })
  }

  const handleRevealToggle = async (entry: SavedSecretEntry) => {
    if (revealedSecretKey === entry.key) {
      clearRevealedPassword()
      return
    }
    const requestId = revealRequestRef.current + 1
    revealRequestRef.current = requestId
    setRevealError(false)
    clearRevealedPassword()
    try {
      const value = await getSavedSecret(entry.key)
      if (revealRequestRef.current !== requestId) return
      setRevealedSecretKey(entry.key)
      setRevealedPassword(value)
      revealTimerRef.current = window.setTimeout(clearRevealedPassword, 10_000)
    } catch {
      if (revealRequestRef.current === requestId) setRevealError(true)
    }
  }

  const handleDeleteSavedSecret = async (entry: SavedSecretEntry) => {
    const confirmed = await confirm({
      title: t("secretStorage.deleteSavedSecret"),
      description: t("secretStorage.deleteSavedSecretConfirm", { label: entry.label }),
      confirmText: t("secretStorage.deleteSavedSecret"),
      cancelText: t("common.cancel"),
      variant: "destructive",
    })
    if (!confirmed) return
    await run("delete", async () => {
      await deleteSavedSecret(entry.key)
      if (revealedSecretKey === entry.key) clearRevealedPassword()
      await reloadSavedSecrets()
      toast({
        title: t("secretStorage.savedSecretDeleted"),
        description: t("secretStorage.savedSecretDeletedDesc", { label: entry.label }),
      })
    })
  }

  const statusLabel = pending
    ? t("secretStorage.statuses.pending")
    : mode === "memory"
      ? t("secretStorage.statuses.disabled")
      : unlocked
        ? t("secretStorage.statuses.unlocked")
        : t("secretStorage.statuses.locked")

  // What the password box above the list is for, if shown at all.
  const unlockPurpose: "migrate" | "setPassword" | "unlock" | "recover" | null = pending
    ? "migrate"
    : mode === "password" && !hasMasterPassword && !choosingPasswordMode
      ? "setPassword"
      : mode === "password" && !unlocked
        ? "unlock"
        : mode === "system" && !unlocked && hasMasterPassword
          ? "recover"
          : null
  const showMasterPasswordCard = !pending && !choosingPasswordMode && unlocked && mode !== "memory"

  return (
    <ScrollArea className="h-full pr-4">
      <div className="space-y-4">
        <SettingsSection
          icon={<Shield size={16} />}
          title={t("secretStorage.title")}
          description={t("secretStorage.description")}
        >
          <SettingsRow
            title={t("secretStorage.status")}
            description={secretStatus.message}
            action={
              <Badge variant={unlocked && mode !== "memory" ? "default" : "secondary"}>
                {statusLabel}
              </Badge>
            }
          />
          <SettingsRow
            title={t("secretStorage.storageMode")}
            description={t(`secretStorage.modeDescriptions.${mode}`)}
          >
            <Label htmlFor="secret-storage-mode" className="sr-only">
              {t("secretStorage.storageMode")}
            </Label>
            <Select
              id="secret-storage-mode"
              value={choosingPasswordMode ? "password" : mode}
              disabled={busy || pending}
              onChange={(event) => void handleModeChange(event.target.value as SecretStorageMode)}
            >
              <option value="system" disabled={!secretStatus.keyringAvailable}>
                {t("secretStorage.modes.system")}
              </option>
              <option value="password">{t("secretStorage.modes.password")}</option>
              <option value="memory">{t("secretStorage.modes.memory")}</option>
            </Select>
          </SettingsRow>
        </SettingsSection>

        {!secretStatus.keyringAvailable && (
          <Alert className="border-amber-500/40 bg-amber-500/10">
            <div className="flex items-start gap-2">
              <AlertTriangle size={16} className="mt-0.5 shrink-0" />
              <div>
                <AlertTitle>{t("secretStorage.keyringUnavailable")}</AlertTitle>
                <AlertDescription className="mt-1 text-xs leading-5">
                  {t("secretStorage.keyringUnavailableDesc")}
                </AlertDescription>
              </div>
            </div>
          </Alert>
        )}

        {choosingPasswordMode && (
          <Card>
            <CardContent className="space-y-3 p-4">
              <div>
                <div className="text-sm font-medium">{t("secretStorage.choosePasswordTitle")}</div>
                <div className="text-muted-foreground mt-1 text-xs leading-5">
                  {t("secretStorage.choosePasswordDesc")}
                </div>
              </div>
              <PasswordField
                id="choose-master-password"
                label={t("secretStorage.newPassword")}
                value={newPassword}
                onChange={setNewPassword}
                disabled={busy}
                autoComplete="new-password"
                autoFocus
              />
              <PasswordField
                id="choose-master-password-confirm"
                label={t("secretStorage.confirmNewPassword")}
                value={confirmPassword}
                onChange={setConfirmPassword}
                disabled={busy}
                autoComplete="new-password"
              />
              <div className="flex gap-2">
                <Button
                  onClick={() => void handleConfirmPasswordMode()}
                  disabled={busy || newPassword.length === 0 || confirmPassword.length === 0}
                >
                  <ActionIcon
                    busy={action === "mode"}
                    icon={<KeyRound size={14} className="mr-2" />}
                  />
                  {t("secretStorage.switchToPassword")}
                </Button>
                <Button
                  variant="outline"
                  disabled={busy}
                  onClick={() => {
                    setChoosingPasswordMode(false)
                    clearPasswordFields()
                  }}
                >
                  {t("secretStorage.cancel")}
                </Button>
              </div>
            </CardContent>
          </Card>
        )}

        {unlockPurpose && (
          <Card>
            <CardContent className="space-y-3 p-4">
              <div>
                <div className="text-sm font-medium">
                  {unlockPurpose === "migrate"
                    ? t("secretStorage.migrationTitle")
                    : unlockPurpose === "setPassword"
                      ? t("secretStorage.setMasterPasswordTitle")
                      : t("secretStorage.unlockTitle")}
                </div>
                {unlockPurpose !== "unlock" && (
                  <div className="text-muted-foreground mt-1 text-xs leading-5">
                    {unlockPurpose === "migrate"
                      ? t("secretStorage.migrationDesc")
                      : unlockPurpose === "setPassword"
                        ? t("secretStorage.setMasterPasswordDesc")
                        : t("secretStorage.unlockRecoveryDesc")}
                  </div>
                )}
              </div>
              <form
                className="space-y-3"
                onSubmit={(event) => {
                  event.preventDefault()
                  void handleUnlock()
                }}
              >
                {unlockPurpose === "setPassword" ? (
                  <>
                    <PasswordField
                      id="vault-new-password"
                      label={t("secretStorage.newPassword")}
                      value={newPassword}
                      onChange={setNewPassword}
                      disabled={busy}
                      autoComplete="new-password"
                    />
                    <PasswordField
                      id="vault-confirm-password"
                      label={t("secretStorage.confirmNewPassword")}
                      value={confirmPassword}
                      onChange={setConfirmPassword}
                      disabled={busy}
                      autoComplete="new-password"
                    />
                  </>
                ) : (
                  <PasswordField
                    id="vault-password"
                    label={t("secretStorage.masterPassword")}
                    value={password}
                    onChange={setPassword}
                    disabled={busy}
                    autoComplete="current-password"
                  />
                )}
                <Button
                  type="submit"
                  disabled={
                    busy ||
                    (unlockPurpose === "setPassword"
                      ? newPassword.length === 0 || confirmPassword.length === 0
                      : password.length === 0)
                  }
                >
                  <ActionIcon
                    busy={action === "unlock"}
                    icon={<Unlock size={14} className="mr-2" />}
                  />
                  {action === "unlock"
                    ? unlockPurpose === "migrate"
                      ? t("secretStorage.migrating")
                      : t("secretStorage.unlocking")
                    : unlockPurpose === "migrate"
                      ? t("secretStorage.migrate")
                      : unlockPurpose === "setPassword"
                        ? t("secretStorage.setPassword")
                        : t("secretStorage.unlock")}
                </Button>
              </form>
            </CardContent>
          </Card>
        )}

        {showMasterPasswordCard && mode === "password" && (
          <Card>
            <CardContent className="flex items-center justify-between gap-4 p-4">
              <div>
                <div className="text-sm font-medium">{t("secretStorage.lock")}</div>
                <div className="text-muted-foreground mt-1 text-xs">
                  {t("secretStorage.lockedDesc")}
                </div>
              </div>
              <Button variant="outline" onClick={() => void handleLock()} disabled={busy}>
                <ActionIcon busy={action === "lock"} icon={<Lock size={14} className="mr-2" />} />
                {t("secretStorage.lock")}
              </Button>
            </CardContent>
          </Card>
        )}

        {showMasterPasswordCard && (hasMasterPassword || mode === "system") && (
          <Card>
            <CardContent className="space-y-3 p-4">
              <div>
                <div className="text-sm font-medium">
                  {mode === "system"
                    ? t("secretStorage.recoveryTitle")
                    : t("secretStorage.changePassword")}
                </div>
                <div className="text-muted-foreground mt-1 text-xs leading-5">
                  {mode === "system"
                    ? t("secretStorage.recoveryDesc")
                    : t("secretStorage.changePasswordDesc")}
                </div>
                {mode === "system" && hasMasterPassword && (
                  <div className="mt-1 text-xs leading-5">{t("secretStorage.recoveryIsSet")}</div>
                )}
              </div>
              {hasMasterPassword && (
                <PasswordField
                  id="current-password"
                  label={t("secretStorage.currentPassword")}
                  value={currentPassword}
                  onChange={setCurrentPassword}
                  disabled={busy}
                  autoComplete="current-password"
                />
              )}
              <PasswordField
                id="new-password"
                label={t("secretStorage.newPassword")}
                value={newPassword}
                onChange={setNewPassword}
                disabled={busy}
                autoComplete="new-password"
              />
              <PasswordField
                id="confirm-password"
                label={t("secretStorage.confirmNewPassword")}
                value={confirmPassword}
                onChange={setConfirmPassword}
                disabled={busy}
                autoComplete="new-password"
              />
              <div className="flex flex-wrap gap-2">
                {hasMasterPassword ? (
                  <Button
                    onClick={() => void handleChangePassword()}
                    disabled={
                      busy ||
                      currentPassword.length === 0 ||
                      newPassword.length === 0 ||
                      confirmPassword.length === 0
                    }
                  >
                    <ActionIcon
                      busy={action === "changePassword"}
                      icon={<KeyRound size={14} className="mr-2" />}
                    />
                    {action === "changePassword"
                      ? t("secretStorage.changingPassword")
                      : t("secretStorage.changePassword")}
                  </Button>
                ) : (
                  <Button
                    onClick={() => void handleSetRecovery()}
                    disabled={busy || newPassword.length === 0 || confirmPassword.length === 0}
                  >
                    <ActionIcon
                      busy={action === "setPassword"}
                      icon={<KeyRound size={14} className="mr-2" />}
                    />
                    {action === "setPassword"
                      ? t("secretStorage.settingPassword")
                      : t("secretStorage.setRecovery")}
                  </Button>
                )}
                {mode === "system" && hasMasterPassword && (
                  <Button
                    variant="outline"
                    className="text-destructive hover:text-destructive"
                    onClick={() => void handleRemoveRecovery()}
                    disabled={busy}
                  >
                    <ActionIcon
                      busy={action === "removePassword"}
                      icon={<Trash2 size={14} className="mr-2" />}
                    />
                    {t("secretStorage.removeRecovery")}
                  </Button>
                )}
              </div>
            </CardContent>
          </Card>
        )}

        {mode === "password" && hasMasterPassword && !pending && (
          <Card>
            <CardContent className="flex items-center justify-between gap-4 p-4">
              <div>
                <div className="text-sm font-medium">
                  {t("secretStorage.promptUnlockOnStartup")}
                </div>
                <div className="text-muted-foreground mt-1 text-xs leading-5">
                  {t("secretStorage.promptUnlockOnStartupDesc")}
                </div>
              </div>
              <Switch
                checked={config.prompt_unlock_vault_on_startup}
                disabled={busy}
                onCheckedChange={(checked) => void handlePromptUnlockOnStartupChange(checked)}
              />
            </CardContent>
          </Card>
        )}

        {error && (
          <Alert className="border-destructive/40 bg-destructive/10 text-destructive">
            <AlertTitle>{t("secretStorage.title")}</AlertTitle>
            <AlertDescription className="mt-1 text-sm text-current" role="alert">
              {error}
            </AlertDescription>
          </Alert>
        )}

        <Card>
          <CardContent className="space-y-3 p-4">
            <div className="flex items-center justify-between gap-3">
              <div>
                <div className="text-sm font-medium">{t("secretStorage.savedPasswords")}</div>
                <div className="text-muted-foreground mt-1 text-xs leading-5">
                  {t("secretStorage.savedPasswordsDesc")}
                </div>
              </div>
              <Badge variant="secondary">
                {t("secretStorage.savedPasswordCount", { count: savedSecrets.length })}
              </Badge>
            </div>

            {savedSecrets.length === 0 ? (
              <p className="text-muted-foreground text-xs leading-5">
                {t("secretStorage.noSavedPasswords")}
              </p>
            ) : (
              <div className="border-border divide-border overflow-hidden rounded-md border">
                {savedSecrets.map((entry) => {
                  const revealed = revealedSecretKey === entry.key
                  const revealLabel = revealed
                    ? t("secretStorage.hideSavedSecret")
                    : t("secretStorage.showSavedSecret")
                  return (
                    <div
                      key={entry.key}
                      className="flex items-center justify-between gap-3 px-3 py-2"
                    >
                      <div className="min-w-0">
                        <div className="truncate text-sm">{entry.label}</div>
                        <div className="text-muted-foreground truncate text-xs">
                          {entry.kind.includes("jump")
                            ? t("secretStorage.secretKinds.jumpHost")
                            : entry.kind === "sudo"
                              ? t("secretStorage.secretKinds.sudo")
                              : t("secretStorage.secretKinds.ssh")}
                        </div>
                        {revealed && revealedPassword !== null && (
                          <div className="bg-muted/60 mt-1 rounded-sm px-2 py-1 font-mono text-xs break-all">
                            {revealedPassword}
                          </div>
                        )}
                      </div>
                      <div className="flex shrink-0 items-center gap-1">
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          disabled={busy || !unlocked}
                          onClick={() => void handleRevealToggle(entry)}
                          title={revealLabel}
                          aria-label={revealLabel}
                        >
                          {revealed ? <EyeOff size={14} /> : <Eye size={14} />}
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          className="text-destructive hover:text-destructive"
                          disabled={busy}
                          onClick={() => void handleDeleteSavedSecret(entry)}
                          title={t("secretStorage.deleteSavedSecret")}
                          aria-label={t("secretStorage.deleteSavedSecret")}
                        >
                          <Trash2 size={14} />
                        </Button>
                      </div>
                    </div>
                  )
                })}
              </div>
            )}
            {revealError && (
              <p className="text-destructive text-xs leading-5">
                {t("secretStorage.revealSavedSecretFailed")}
              </p>
            )}
          </CardContent>
        </Card>
      </div>
    </ScrollArea>
  )
}
