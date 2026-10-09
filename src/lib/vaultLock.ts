import { toErrorMessage } from "@/lib/utils"

/** Matches `LOCKED` in src-tauri/src/ssh/secret_store/mod.rs. */
export const VAULT_LOCKED = "Saved passwords are locked. Unlock them first."
/** Asks the app to offer unlocking saved passwords; carries a `VaultLockedDetail`. */
export const VAULT_LOCKED_EVENT = "tterm:vault-locked"

export interface VaultLockedDetail {
  /** Runs once saved passwords are unlocked. */
  retry?: () => void
}

interface VaultStatus {
  storageMode: string
  hasMasterPassword: boolean
  migrationPending: boolean
}

export function isVaultLocked(error: unknown): boolean {
  return toErrorMessage(error).includes(VAULT_LOCKED)
}

/** A password typed here can unlock saved passwords; otherwise Settings > Security has to. */
export function vaultUnlockable(status: VaultStatus): boolean {
  return status.storageMode !== "memory" && (status.migrationPending || status.hasMasterPassword)
}

/** Offers to unlock saved passwords without interrupting, then runs `retry`. */
export function reportVaultLocked(retry?: () => void) {
  window.dispatchEvent(
    new CustomEvent<VaultLockedDetail>(VAULT_LOCKED_EVENT, { detail: { retry } })
  )
}
