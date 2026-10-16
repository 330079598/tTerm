import { useCallback, useEffect, useState } from "react"
import { invoke } from "@tauri-apps/api/core"

import type { ImportVaultKeyInput, VaultKeySummary } from "@/lib/keyVault"
import { onSyncApplied } from "@/lib/sync"
import { toErrorMessage } from "@/lib/utils"

/** Sent after a key was imported, so every open list shows it. */
const VAULT_KEYS_CHANGED_EVENT = "tterm:vault-keys-changed"

/**
 * The keys in the SSH key vault, loaded when mounted and again after a sync
 * or an import elsewhere in the app changed them. `loadError` is set while
 * the vault cannot be read, for instance when saved passwords are locked.
 */
export function useVaultKeys() {
  const [keys, setKeys] = useState<VaultKeySummary[]>([])
  const [loaded, setLoaded] = useState(false)
  const [loadError, setLoadError] = useState<string | null>(null)

  // The state is set in the promise callbacks rather than after an `await`,
  // which the effect lint can't yet tell apart from setting it synchronously.
  const reload = useCallback(
    () =>
      invoke<VaultKeySummary[]>("list_vault_keys").then(
        (entries) => {
          setKeys(entries)
          setLoadError(null)
          setLoaded(true)
        },
        (error: unknown) => {
          setKeys([])
          setLoadError(toErrorMessage(error))
          setLoaded(true)
        }
      ),
    []
  )

  useEffect(() => {
    void reload()
    const onChanged = () => void reload()
    window.addEventListener(VAULT_KEYS_CHANGED_EVENT, onChanged)
    const stopSync = onSyncApplied(["secrets", "profiles"], onChanged)
    return () => {
      window.removeEventListener(VAULT_KEYS_CHANGED_EVENT, onChanged)
      stopSync()
    }
  }, [reload])

  return { keys, loaded, loadError, reload }
}

/** Imports a key file into the vault and tells every open key list. */
export async function importVaultKey(input: ImportVaultKeyInput): Promise<VaultKeySummary> {
  const key = await invoke<VaultKeySummary>("import_vault_key", { input })
  announceVaultKeysChanged()
  return key
}

/** Tells every open key list to load the vault again. */
export function announceVaultKeysChanged() {
  window.dispatchEvent(new Event(VAULT_KEYS_CHANGED_EVENT))
}
