import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react"
import { invoke } from "@tauri-apps/api/core"
import { Loader2, ShieldCheck } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { PasswordInput } from "@/components/ui/password-input"
import { Label } from "@/components/ui/label"
import { ToastAction } from "@/components/ui/toast"
import { VaultStartupUnlockDialog } from "@/components/VaultStartupUnlockDialog"
import { useConfig } from "@/contexts/ConfigContext"
import { toast } from "@/hooks/use-toast"
import { useStableRef } from "@/hooks/useStableRef"
import {
  SYSTEM_VERIFICATION_CANCELED,
  UserVerificationCanceledError,
  verificationRequired,
  type VerificationPurpose,
} from "@/lib/userVerification"
import { toErrorMessage } from "@/lib/utils"
import {
  isVaultLocked,
  vaultUnlockable,
  VAULT_LOCKED_EVENT,
  type VaultLockedDetail,
} from "@/lib/vaultLock"

/** Matches `WRONG_MASTER_PASSWORD` in src-tauri/src/ssh/secret_store/mod.rs. */
const WRONG_MASTER_PASSWORD = "Incorrect master password."

interface UserVerificationContextValue {
  /**
   * Runs `action`; when the backend asks for verification first, verifies
   * the user (Windows Hello, Touch ID or the master password) and runs it once more.
   * Throws `UserVerificationCanceledError` when the user backs out.
   */
  withVerification: <T>(purpose: VerificationPurpose, action: () => Promise<T>) => Promise<T>
  /**
   * Runs `action`; when saved passwords turn out to be locked, asks for the
   * master password and runs it once more. Throws `UserVerificationCanceledError`
   * when the user backs out.
   */
  withVaultUnlock: <T>(action: () => Promise<T>) => Promise<T>
  /** Asks for the master password; false when canceled or it cannot unlock here. */
  requestVaultUnlock: () => Promise<boolean>
}

interface PasswordRequest {
  purpose: VerificationPurpose
  resolve: (verified: boolean) => void
}

const UserVerificationContext = createContext<UserVerificationContextValue | null>(null)

export function UserVerificationProvider({ children }: { children: React.ReactNode }) {
  const { t } = useTranslation()
  const { secretStatus } = useConfig()
  const secretStatusRef = useStableRef(secretStatus)
  const [unlockOpen, setUnlockOpen] = useState(false)
  // Everyone waiting on the one unlock prompt.
  const unlockWaitersRef = useRef<((unlocked: boolean) => void)[]>([])
  // Work that failed in the background while locked, run once unlocked.
  const lockedRetriesRef = useRef<(() => void)[]>([])
  const lockedToastRef = useRef<{ dismiss: () => void } | null>(null)
  const [request, setRequest] = useState<PasswordRequest | null>(null)
  const [password, setPassword] = useState("")
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const requestRef = useRef<PasswordRequest | null>(null)

  const askForPassword = useCallback(
    (purpose: VerificationPurpose) =>
      new Promise<boolean>((resolve) => {
        // A prompt already open for something else is given up.
        requestRef.current?.resolve(false)
        const next = { purpose, resolve }
        requestRef.current = next
        setPassword("")
        setError(null)
        setRequest(next)
      }),
    []
  )

  const finish = useCallback((verified: boolean) => {
    requestRef.current?.resolve(verified)
    requestRef.current = null
    setRequest(null)
    setPassword("")
    setError(null)
  }, [])

  const requestVaultUnlock = useCallback(() => {
    if (!vaultUnlockable(secretStatusRef.current)) return Promise.resolve(false)
    return new Promise<boolean>((resolve) => {
      unlockWaitersRef.current.push(resolve)
      setUnlockOpen(true)
    })
  }, [secretStatusRef])

  const finishUnlock = useCallback((unlocked: boolean) => {
    const waiters = unlockWaitersRef.current
    unlockWaitersRef.current = []
    setUnlockOpen(false)
    for (const resolve of waiters) resolve(unlocked)
  }, [])

  const withVaultUnlock = useCallback(
    async <T,>(action: () => Promise<T>): Promise<T> => {
      try {
        return await action()
      } catch (error) {
        if (!isVaultLocked(error) || !vaultUnlockable(secretStatusRef.current)) throw error
        if (!(await requestVaultUnlock())) {
          throw new UserVerificationCanceledError(t("secretStorage.unlockCanceled"))
        }
        return action()
      }
    },
    [requestVaultUnlock, secretStatusRef, t]
  )

  // Once unlocked, by the prompt or anywhere else, what failed meanwhile runs again.
  useEffect(() => {
    if (!secretStatus.unlocked) return
    lockedToastRef.current?.dismiss()
    lockedToastRef.current = null
    const retries = lockedRetriesRef.current
    lockedRetriesRef.current = []
    for (const retry of retries) retry()
  }, [secretStatus.unlocked])

  useEffect(() => {
    const handleLocked = (event: Event) => {
      const { retry } = (event as CustomEvent<VaultLockedDetail>).detail ?? {}
      if (retry) lockedRetriesRef.current.push(retry)
      // Several tabs failing together share one notice.
      lockedToastRef.current?.dismiss()
      const unlockable = vaultUnlockable(secretStatusRef.current)
      lockedToastRef.current = toast({
        title: t("secretStorage.lockedPromptTitle"),
        description: unlockable
          ? t("secretStorage.lockedPromptDesc")
          : t("secretStorage.lockedPromptSettingsDesc"),
        variant: "destructive",
        duration: Number.POSITIVE_INFINITY,
        action: unlockable ? (
          <ToastAction
            altText={t("secretStorage.unlock")}
            onClick={() => void requestVaultUnlock()}
          >
            {t("secretStorage.unlock")}
          </ToastAction>
        ) : undefined,
      })
    }
    window.addEventListener(VAULT_LOCKED_EVENT, handleLocked)
    return () => window.removeEventListener(VAULT_LOCKED_EVENT, handleLocked)
  }, [requestVaultUnlock, secretStatusRef, t])

  const verify = useCallback(
    async <T,>(purpose: VerificationPurpose, action: () => Promise<T>): Promise<T> => {
      try {
        return await action()
      } catch (error) {
        const method = verificationRequired(error)
        if (!method) throw error
        if (method === "system") {
          try {
            await invoke("verify_user", { input: { purpose } })
          } catch (verifyError) {
            if (toErrorMessage(verifyError) === SYSTEM_VERIFICATION_CANCELED) {
              throw new UserVerificationCanceledError(t("userVerification.canceled"))
            }
            throw verifyError
          }
        } else if (!(await askForPassword(purpose))) {
          throw new UserVerificationCanceledError(t("userVerification.canceled"))
        }
        return action()
      }
    },
    [askForPassword, t]
  )

  // Showing or changing saved passwords also needs them unlocked.
  const withVerification = useCallback(
    <T,>(purpose: VerificationPurpose, action: () => Promise<T>): Promise<T> =>
      withVaultUnlock(() => verify(purpose, action)),
    [verify, withVaultUnlock]
  )

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault()
    if (!request || !password) return
    setBusy(true)
    setError(null)
    try {
      await invoke("verify_user", { input: { purpose: request.purpose, password } })
      finish(true)
    } catch (verifyError) {
      const message = toErrorMessage(verifyError)
      setError(message === WRONG_MASTER_PASSWORD ? t("userVerification.wrongPassword") : message)
    } finally {
      setBusy(false)
    }
  }

  const value = useMemo(
    () => ({ withVerification, withVaultUnlock, requestVaultUnlock }),
    [withVerification, withVaultUnlock, requestVaultUnlock]
  )

  return (
    <UserVerificationContext.Provider value={value}>
      {children}
      <VaultStartupUnlockDialog
        open={unlockOpen}
        variant="action"
        onClose={() => finishUnlock(false)}
        onUnlocked={() => finishUnlock(true)}
      />
      <Dialog open={request !== null} onOpenChange={(open) => !open && !busy && finish(false)}>
        <DialogContent className="sm:max-w-md">
          <form className="space-y-4" onSubmit={handleSubmit}>
            <DialogHeader>
              <DialogTitle className="flex items-center gap-2">
                <ShieldCheck className="size-4" />
                {t("userVerification.title")}
              </DialogTitle>
              <DialogDescription>
                {request?.purpose === "reveal"
                  ? t("userVerification.revealDesc")
                  : t("userVerification.sensitiveDesc")}
              </DialogDescription>
            </DialogHeader>

            <div className="space-y-2">
              <Label htmlFor="user-verification-password">
                {t("secretStorage.masterPassword")}
              </Label>
              <PasswordInput
                id="user-verification-password"
                autoFocus
                autoComplete="current-password"
                value={password}
                placeholder={t("secretStorage.masterPasswordPlaceholder")}
                disabled={busy}
                onChange={(event) => setPassword(event.target.value)}
              />
              {error && (
                <p className="text-destructive mt-1 text-xs" role="alert">
                  {error}
                </p>
              )}
            </div>

            <DialogFooter>
              <Button type="button" variant="outline" disabled={busy} onClick={() => finish(false)}>
                {t("common.cancel")}
              </Button>
              <Button type="submit" disabled={busy || password.length === 0}>
                {busy ? (
                  <Loader2 size={14} className="mr-2 animate-spin" />
                ) : (
                  <ShieldCheck size={14} className="mr-2" />
                )}
                {t("userVerification.verify")}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </UserVerificationContext.Provider>
  )
}

export function useUserVerification() {
  const context = useContext(UserVerificationContext)
  if (!context) {
    throw new Error("useUserVerification must be used within UserVerificationProvider")
  }
  return context
}
