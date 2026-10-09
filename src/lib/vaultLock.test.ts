// @vitest-environment jsdom
import { describe, expect, it, vi } from "vitest"

import {
  isVaultLocked,
  reportVaultLocked,
  vaultUnlockable,
  VAULT_LOCKED_EVENT,
  type VaultLockedDetail,
} from "@/lib/vaultLock"

describe("isVaultLocked", () => {
  it("recognizes the backend's locked error", () => {
    expect(isVaultLocked("Saved passwords are locked. Unlock them first.")).toBe(true)
    expect(isVaultLocked(new Error("Saved passwords are locked. Unlock them first."))).toBe(true)
    expect(isVaultLocked("Incorrect master password.")).toBe(false)
    expect(isVaultLocked(undefined)).toBe(false)
  })
})

describe("vaultUnlockable", () => {
  const status = { storageMode: "password", hasMasterPassword: true, migrationPending: false }

  it("needs a password that can unlock", () => {
    expect(vaultUnlockable(status)).toBe(true)
    expect(vaultUnlockable({ ...status, storageMode: "system" })).toBe(true)
    expect(vaultUnlockable({ ...status, hasMasterPassword: false })).toBe(false)
    expect(vaultUnlockable({ ...status, hasMasterPassword: false, migrationPending: true })).toBe(
      true
    )
    expect(vaultUnlockable({ ...status, storageMode: "memory" })).toBe(false)
  })
})

describe("reportVaultLocked", () => {
  it("sends the retry with the event", () => {
    const retry = vi.fn()
    const listener = vi.fn((event: Event) =>
      (event as CustomEvent<VaultLockedDetail>).detail.retry?.()
    )
    window.addEventListener(VAULT_LOCKED_EVENT, listener)
    reportVaultLocked(retry)
    window.removeEventListener(VAULT_LOCKED_EVENT, listener)
    expect(listener).toHaveBeenCalledOnce()
    expect(retry).toHaveBeenCalledOnce()
  })
})
