import React, { useCallback, useEffect, useRef, useState } from "react"
import { AlertTriangle, Lock } from "lucide-react"
import { useTranslation } from "react-i18next"

import { ActionIcon } from "@/components/SettingsDialog/SecretFields"
import { UnlockVaultCard } from "@/components/SettingsDialog/UnlockVaultCard"
import { SecretStorageSection } from "@/components/SettingsDialog/SecretStorageSection"
import { ChoosePasswordModeCard } from "@/components/SettingsDialog/ChoosePasswordModeCard"
import { SystemUnlockFailedCard } from "@/components/SettingsDialog/SystemUnlockFailedCard"
import { MasterPasswordCard } from "@/components/SettingsDialog/MasterPasswordCard"
import { SavedSecretsCard } from "@/components/SettingsDialog/SavedSecretsCard"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Switch } from "@/components/ui/switch"
import { KnownHostsCard } from "@/components/SettingsDialog/KnownHostsCard"
import type { VaultAction } from "@/components/SettingsDialog/types"
import type { useConfirmDialog, useInfoDialog } from "@/components/ui/app-dialog"
import { useConfig } from "@/contexts/ConfigContext"
import type { SavedSecretEntry, SecretStorageMode } from "@/lib/appConfig"
import { useUserVerification } from "@/contexts/UserVerificationContext"
import { useToast } from "@/hooks/use-toast"
import { isVerificationCanceled } from "@/lib/userVerification"
import { toErrorMessage } from "@/lib/utils"

interface SecuritySettingsTabProps {
  confirm: ReturnType<typeof useConfirmDialog>["confirm"]
  info: ReturnType<typeof useInfoDialog>["info"]
}

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
    retrySystemUnlock,
    resetSavedPasswords,
    changeVaultPassword,
    setMasterPassword,
    removeMasterPassword,
    listSavedSecrets,
    getSavedSecret,
    deleteSavedSecret,
  } = useConfig()
  const { withVerification } = useUserVerification()

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
  const [revealError, setRevealError] = useState<string | null>(null)
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
    // eslint-disable-next-line react-hooks/set-state-in-effect -- state is only set after await; false positive fixed upstream in facebook/react#36734
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
      if (isMountedRef.current && !isVerificationCanceled(caught)) {
        setError(toErrorMessage(caught))
      }
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

  const handleRetrySystemUnlock = async () => {
    await run("retry", async () => {
      await retrySystemUnlock()
      toast({
        title: t("secretStorage.unlocked"),
        description: t("secretStorage.unlockedDesc"),
        variant: "success",
      })
      await reloadSavedSecrets()
    })
  }

  const handleResetSavedPasswords = async () => {
    const confirmed = await confirm({
      title: t("secretStorage.resetTitle"),
      description: t("secretStorage.resetConfirm"),
      confirmText: t("secretStorage.reset"),
      cancelText: t("common.cancel"),
      variant: "destructive",
    })
    if (!confirmed) return
    clearRevealedPassword()
    await run("reset", async () => {
      await resetSavedPasswords()
      toast({
        title: t("secretStorage.resetDone"),
        description: t("secretStorage.resetDoneDesc"),
        variant: "success",
      })
      await reloadSavedSecrets()
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
      await withVerification("sensitive", removeMasterPassword)
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
    setRevealError(null)
    clearRevealedPassword()
    try {
      const value = await withVerification("reveal", () => getSavedSecret(entry.key))
      if (revealRequestRef.current !== requestId) return
      setRevealedSecretKey(entry.key)
      setRevealedPassword(value)
      revealTimerRef.current = window.setTimeout(clearRevealedPassword, 10_000)
    } catch (caught) {
      if (revealRequestRef.current !== requestId || isVerificationCanceled(caught)) return
      setRevealError(unlocked ? toErrorMessage(caught) : t("secretStorage.revealSavedSecretFailed"))
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
  // Locked in automatic mode: the credential store key is missing, does not
  // match, or did not answer. Without the recovery password nothing else here
  // can unlock, so offer to try again or start over.
  const systemUnlockFailed = !pending && mode === "system" && !unlocked
  const showMasterPasswordCard = !pending && !choosingPasswordMode && unlocked && mode !== "memory"

  return (
    <ScrollArea className="h-full pr-4">
      <div className="space-y-4">
        <SecretStorageSection
          busy={busy}
          mode={mode}
          unlocked={unlocked}
          pending={pending}
          handleModeChange={handleModeChange}
          secretStatus={secretStatus}
          choosingPasswordMode={choosingPasswordMode}
          statusLabel={statusLabel}
        />

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
          <ChoosePasswordModeCard
            busy={busy}
            action={action}
            newPassword={newPassword}
            confirmPassword={confirmPassword}
            setNewPassword={setNewPassword}
            setConfirmPassword={setConfirmPassword}
            handleConfirmPasswordMode={handleConfirmPasswordMode}
            clearPasswordFields={clearPasswordFields}
            setChoosingPasswordMode={setChoosingPasswordMode}
          />
        )}

        {unlockPurpose && (
          <UnlockVaultCard
            unlockPurpose={unlockPurpose}
            busy={busy}
            action={action}
            password={password}
            newPassword={newPassword}
            confirmPassword={confirmPassword}
            setPassword={setPassword}
            setNewPassword={setNewPassword}
            setConfirmPassword={setConfirmPassword}
            handleUnlock={handleUnlock}
          />
        )}

        {systemUnlockFailed && (
          <SystemUnlockFailedCard
            busy={busy}
            action={action}
            hasMasterPassword={hasMasterPassword}
            handleRetrySystemUnlock={handleRetrySystemUnlock}
            handleResetSavedPasswords={handleResetSavedPasswords}
            secretStatus={secretStatus}
          />
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
          <MasterPasswordCard
            busy={busy}
            action={action}
            mode={mode}
            hasMasterPassword={hasMasterPassword}
            currentPassword={currentPassword}
            newPassword={newPassword}
            confirmPassword={confirmPassword}
            setCurrentPassword={setCurrentPassword}
            setNewPassword={setNewPassword}
            setConfirmPassword={setConfirmPassword}
            handleChangePassword={handleChangePassword}
            handleSetRecovery={handleSetRecovery}
            handleRemoveRecovery={handleRemoveRecovery}
          />
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

        <SavedSecretsCard
          busy={busy}
          mode={mode}
          unlocked={unlocked}
          handleRevealToggle={handleRevealToggle}
          handleDeleteSavedSecret={handleDeleteSavedSecret}
          savedSecrets={savedSecrets}
          revealedSecretKey={revealedSecretKey}
          revealedPassword={revealedPassword}
          revealError={revealError}
          secretStatus={secretStatus}
        />

        <KnownHostsCard confirm={confirm} />
      </div>
    </ScrollArea>
  )
}
