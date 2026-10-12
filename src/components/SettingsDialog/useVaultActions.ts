import { useState } from "react"
import { useTranslation } from "react-i18next"

import type { VaultAction } from "@/components/SettingsDialog/types"
import { useSavedSecrets } from "@/components/SettingsDialog/useSavedSecrets"
import type { useConfirmDialog, useInfoDialog } from "@/components/ui/app-dialog"
import { useConfig } from "@/contexts/ConfigContext"
import { useUserVerification } from "@/contexts/UserVerificationContext"
import { useToast } from "@/hooks/use-toast"
import type { SavedSecretEntry, SecretStorageMode } from "@/lib/appConfig"
import { isVerificationCanceled } from "@/lib/userVerification"
import { toErrorMessage } from "@/lib/utils"

interface VaultActionsOptions {
  confirm: ReturnType<typeof useConfirmDialog>["confirm"]
  info: ReturnType<typeof useInfoDialog>["info"]
}

/**
 * The security settings' password fields and what their buttons do: choosing
 * the storage mode, unlocking, locking, the master password, and deleting a
 * saved password.
 */
export function useVaultActions({ confirm, info }: VaultActionsOptions) {
  const { t } = useTranslation()
  const { toast } = useToast()
  const {
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
  const busy = action !== null
  const mode = secretStatus.storageMode
  const pending = secretStatus.migrationPending
  const { unlocked, hasMasterPassword } = secretStatus
  const {
    savedSecrets,
    revealedSecretKey,
    revealedPassword,
    revealError,
    isMountedRef,
    reloadSavedSecrets,
    clearRevealedPassword,
    handleRevealToggle,
  } = useSavedSecrets(unlocked)

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

  return {
    password,
    setPassword,
    currentPassword,
    setCurrentPassword,
    newPassword,
    setNewPassword,
    confirmPassword,
    setConfirmPassword,
    choosingPasswordMode,
    setChoosingPasswordMode,
    action,
    busy,
    error,
    savedSecrets,
    revealedSecretKey,
    revealedPassword,
    revealError,
    clearPasswordFields,
    handleModeChange,
    handleConfirmPasswordMode,
    handleUnlock,
    handleLock,
    handleRetrySystemUnlock,
    handleResetSavedPasswords,
    handleChangePassword,
    handleSetRecovery,
    handleRemoveRecovery,
    handlePromptUnlockOnStartupChange,
    handleRevealToggle,
    handleDeleteSavedSecret,
  }
}
