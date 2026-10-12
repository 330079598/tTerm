import { useState } from "react"

import { useConfig } from "@/contexts/ConfigContext"

/**
 * Whether to ask for a password at startup to unlock saved passwords, and
 * whether SSH connections may start yet.
 */
export function useStartupVaultUnlock() {
  const { config, isLoaded, isSecretStatusLoaded, secretStatus } = useConfig()
  const [startupVaultUnlockDismissed, setStartupVaultUnlockDismissed] = useState(false)
  // Ask for the password when saved passwords are locked but one can unlock
  // them: the old vault's password to finish moving it, the master password,
  // or the recovery password when the system credential store key is lost.
  const shouldPromptStartupVaultUnlock =
    isLoaded &&
    !secretStatus.unlocked &&
    !startupVaultUnlockDismissed &&
    (secretStatus.migrationPending ||
      (secretStatus.hasMasterPassword &&
        (secretStatus.storageMode === "system" ||
          (secretStatus.storageMode === "password" && config.prompt_unlock_vault_on_startup))))
  // SSH connections wait for startup to unlock saved passwords, then for the prompt.
  const startupConnectionsReady = isSecretStatusLoaded && !shouldPromptStartupVaultUnlock

  return {
    shouldPromptStartupVaultUnlock,
    startupConnectionsReady,
    dismissStartupVaultUnlock: () => setStartupVaultUnlockDismissed(true),
  }
}
