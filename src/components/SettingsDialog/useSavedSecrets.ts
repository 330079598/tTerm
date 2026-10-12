import { useCallback, useEffect, useRef, useState } from "react"
import { useTranslation } from "react-i18next"

import { useConfig } from "@/contexts/ConfigContext"
import { useUserVerification } from "@/contexts/UserVerificationContext"
import type { SavedSecretEntry } from "@/lib/appConfig"
import { isVerificationCanceled } from "@/lib/userVerification"
import { toErrorMessage } from "@/lib/utils"

/**
 * The saved passwords listed in the security settings, and the one shown in
 * plain text for ten seconds after the user verified it is them.
 */
export function useSavedSecrets(unlocked: boolean) {
  const { t } = useTranslation()
  const { listSavedSecrets, getSavedSecret } = useConfig()
  const { withVerification } = useUserVerification()
  const [savedSecrets, setSavedSecrets] = useState<SavedSecretEntry[]>([])
  const [revealedSecretKey, setRevealedSecretKey] = useState<string | null>(null)
  const [revealedPassword, setRevealedPassword] = useState<string | null>(null)
  const [revealError, setRevealError] = useState<string | null>(null)
  const revealTimerRef = useRef<number | null>(null)
  const revealRequestRef = useRef(0)
  const isMountedRef = useRef(true)

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

  return {
    savedSecrets,
    revealedSecretKey,
    revealedPassword,
    revealError,
    /** False once the settings page has gone, so late results are dropped. */
    isMountedRef,
    reloadSavedSecrets,
    clearRevealedPassword,
    handleRevealToggle,
  }
}
