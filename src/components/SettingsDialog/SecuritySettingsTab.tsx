import React from "react"
import { useTranslation } from "react-i18next"

import {
  KeyringUnavailableAlert,
  LockVaultCard,
  StartupUnlockPromptCard,
  VaultErrorAlert,
} from "@/components/SettingsDialog/SecurityCards"
import { UnlockVaultCard } from "@/components/SettingsDialog/UnlockVaultCard"
import { SecretStorageSection } from "@/components/SettingsDialog/SecretStorageSection"
import { ChoosePasswordModeCard } from "@/components/SettingsDialog/ChoosePasswordModeCard"
import { SystemUnlockFailedCard } from "@/components/SettingsDialog/SystemUnlockFailedCard"
import { MasterPasswordCard } from "@/components/SettingsDialog/MasterPasswordCard"
import { SavedSecretsCard } from "@/components/SettingsDialog/SavedSecretsCard"
import { KnownHostsCard } from "@/components/SettingsDialog/KnownHostsCard"
import { useVaultActions } from "@/components/SettingsDialog/useVaultActions"
import { ScrollArea } from "@/components/ui/scroll-area"
import type { useConfirmDialog, useInfoDialog } from "@/components/ui/app-dialog"
import { useConfig } from "@/contexts/ConfigContext"

interface SecuritySettingsTabProps {
  confirm: ReturnType<typeof useConfirmDialog>["confirm"]
  info: ReturnType<typeof useInfoDialog>["info"]
}

export const SecuritySettingsTab: React.FC<SecuritySettingsTabProps> = ({ confirm, info }) => {
  const { t } = useTranslation()
  const { config, secretStatus } = useConfig()
  const {
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
  } = useVaultActions({ confirm, info })
  const mode = secretStatus.storageMode
  const pending = secretStatus.migrationPending
  const { unlocked, hasMasterPassword } = secretStatus

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

        {!secretStatus.keyringAvailable && <KeyringUnavailableAlert />}

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
          <LockVaultCard busy={busy} locking={action === "lock"} onLock={() => void handleLock()} />
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
          <StartupUnlockPromptCard
            checked={config.prompt_unlock_vault_on_startup}
            busy={busy}
            onCheckedChange={(checked) => void handlePromptUnlockOnStartupChange(checked)}
          />
        )}

        {error && <VaultErrorAlert error={error} />}

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
