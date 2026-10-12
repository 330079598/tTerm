import { invoke } from "@tauri-apps/api/core"
import { useCallback, useState, type RefObject } from "react"

import {
  type AppConfig,
  type SavedSecretEntry,
  type SecretBackendStatus,
  type SecretStorageMode,
  defaultSecretStatus,
} from "@/lib/appConfig"
import { normalizeSecretStatus } from "@/lib/appConfigNormalize"

interface SecretStoreOptions {
  configRef: RefObject<AppConfig>
  updateConfigState: (update: (current: AppConfig) => AppConfig) => void
}

/**
 * Where saved passwords stand and what can be done to them. Each action
 * returns the status it leaves, which also updates the storage mode in the
 * config.
 */
export function useSecretStore({ configRef, updateConfigState }: SecretStoreOptions) {
  const [secretStatus, setSecretStatus] = useState<SecretBackendStatus>(defaultSecretStatus)
  const [isSecretStatusLoaded, setIsSecretStatusLoaded] = useState(false)

  const refreshSecretStatus = useCallback(async (): Promise<SecretBackendStatus> => {
    try {
      const status = await invoke<SecretBackendStatus>("get_secret_backend_status")
      const normalized = normalizeSecretStatus(status)
      setSecretStatus(normalized)
      return normalized
    } catch (error) {
      console.error("Failed to load secret backend status:", error)
      setSecretStatus(defaultSecretStatus)
      return defaultSecretStatus
    } finally {
      setIsSecretStatusLoaded(true)
    }
  }, [])

  const applySecretStatus = useCallback(
    (status: SecretBackendStatus) => {
      const normalized = normalizeSecretStatus(status)
      setSecretStatus(normalized)
      updateConfigState((prev) => ({ ...prev, secret_storage_mode: normalized.storageMode }))
      return normalized
    },
    [updateConfigState]
  )

  const setSecretStorageMode = useCallback(
    async (mode: SecretStorageMode, password?: string) => {
      const switchingToPassword =
        mode === "password" && configRef.current.secret_storage_mode !== "password"
      const status = applySecretStatus(
        await invoke<SecretBackendStatus>("set_secret_storage_mode", {
          input: { mode, password: password || null },
        })
      )
      if (switchingToPassword) {
        // The backend turns the startup prompt on with master password mode.
        updateConfigState((prev) => ({ ...prev, prompt_unlock_vault_on_startup: true }))
      }
      return status
    },
    [applySecretStatus, configRef, updateConfigState]
  )

  const unlockSecretVault = useCallback(
    async (password: string) =>
      applySecretStatus(
        await invoke<SecretBackendStatus>("unlock_secret_vault", { input: { password } })
      ),
    [applySecretStatus]
  )

  const lockSecretVault = useCallback(
    async () => applySecretStatus(await invoke<SecretBackendStatus>("lock_secret_vault")),
    [applySecretStatus]
  )

  const retrySystemUnlock = useCallback(
    async () => applySecretStatus(await invoke<SecretBackendStatus>("retry_system_unlock")),
    [applySecretStatus]
  )

  const resetSavedPasswords = useCallback(
    async () => applySecretStatus(await invoke<SecretBackendStatus>("reset_saved_passwords")),
    [applySecretStatus]
  )

  const changeVaultPassword = useCallback(
    async (currentPassword: string, newPassword: string) =>
      applySecretStatus(
        await invoke<SecretBackendStatus>("change_vault_password", {
          input: { currentPassword, newPassword },
        })
      ),
    [applySecretStatus]
  )

  const setMasterPassword = useCallback(
    async (password: string) =>
      applySecretStatus(
        await invoke<SecretBackendStatus>("set_master_password", { input: { password } })
      ),
    [applySecretStatus]
  )

  const removeMasterPassword = useCallback(
    async () => applySecretStatus(await invoke<SecretBackendStatus>("remove_master_password")),
    [applySecretStatus]
  )

  const listSavedSecrets = useCallback(
    async () => invoke<SavedSecretEntry[]>("list_saved_secrets"),
    []
  )

  const getSavedSecret = useCallback(
    async (key: string) => invoke<string>("get_saved_secret", { input: { key } }),
    []
  )

  const deleteSavedSecret = useCallback(
    async (key: string) => invoke<boolean>("delete_saved_secret", { input: { key } }),
    []
  )

  return {
    secretStatus,
    isSecretStatusLoaded,
    refreshSecretStatus,
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
  }
}
