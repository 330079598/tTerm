import React, { useEffect, useRef, useState } from "react"
import { invoke } from "@tauri-apps/api/core"

import { getJumpHostPasswordLookupKey } from "@/components/ConnectionDialog/jumpHostPayload"
import type {
  ConnectionDialogContentProps,
  ConnectionForm,
} from "@/components/ConnectionDialog/types"

interface UseSavedPasswordAvailabilityOptions {
  editProfile: ConnectionDialogContentProps["editProfile"]
  draftProfile: ConnectionDialogContentProps["draftProfile"]
  typedPasswordTabId: ConnectionDialogContentProps["typedPasswordTabId"]
  form: ConnectionForm
  setForm: React.Dispatch<React.SetStateAction<ConnectionForm>>
}

/**
 * Which passwords the profile already has saved (or the quick connection
 * typed), so the form can say so instead of asking for them again.
 */
export function useSavedPasswordAvailability({
  editProfile,
  draftProfile,
  typedPasswordTabId,
  form,
  setForm,
}: UseSavedPasswordAvailabilityOptions) {
  const [savedPasswordAvailable, setSavedPasswordAvailable] = useState(false)
  // The password typed while the quick connection being saved signed in.
  const [typedPasswordAvailable, setTypedPasswordAvailable] = useState(false)
  const [savedSudoPasswordAvailable, setSavedSudoPasswordAvailable] = useState(false)
  const [savedJumpPasswordKeys, setSavedJumpPasswordKeys] = useState<Set<string>>(() => new Set())
  const loadedJumpPasswordsForProfile = useRef<string | null>(null)

  useEffect(() => {
    if (!editProfile || form.type !== "ssh" || form.authMethod !== "password") {
      return
    }

    let cancelled = false

    invoke<boolean>("has_saved_password", {
      profileId: editProfile.id,
      profileName: editProfile.name,
    })
      .then((hasPassword) => {
        if (cancelled || !hasPassword) {
          return
        }

        setSavedPasswordAvailable(true)
        setForm((current) => {
          if (current.type !== "ssh" || current.authMethod !== "password") {
            return current
          }

          return {
            ...current,
            rememberPassword: true,
          }
        })
      })
      .catch(() => {})

    return () => {
      cancelled = true
    }
  }, [editProfile, form.authMethod, form.type, setForm])

  // Offer to store what was typed while the quick connection signed in. The
  // passwords stay in the backend; saving names the tab to take them from.
  useEffect(() => {
    if (!typedPasswordTabId || !draftProfile) return
    const hasTyped = (account: { host: string; port: number; username: string }) =>
      invoke<boolean>("has_typed_password", { tabId: typedPasswordTabId, ...account }).catch(
        () => false
      )
    const { host, username } = draftProfile
    const passwordJumps = (draftProfile.jump_hosts ?? []).filter(
      (jump) => jump.auth_method === "password"
    )
    let cancelled = false
    Promise.all([
      draftProfile.auth_method === "password" && host && username
        ? hasTyped({ host, port: draftProfile.port ?? 22, username })
        : false,
      Promise.all(passwordJumps.map((jump) => hasTyped(jump))),
    ]).then(([targetTyped, jumpsTyped]) => {
      if (cancelled || !(targetTyped || jumpsTyped.some(Boolean))) return
      setTypedPasswordAvailable(targetTyped)
      setSavedJumpPasswordKeys(
        new Set(
          passwordJumps
            .filter((_, index) => jumpsTyped[index])
            .map((jump) => getJumpHostPasswordLookupKey(jump))
        )
      )
      setForm((current) => ({ ...current, rememberPassword: true }))
    })
    return () => {
      cancelled = true
    }
  }, [draftProfile, typedPasswordTabId, setForm])

  useEffect(() => {
    if (!editProfile || form.type !== "ssh") {
      return
    }

    let cancelled = false
    invoke<boolean>("has_saved_sudo_password", { profileId: editProfile.id })
      .then((hasPassword) => {
        if (!cancelled) setSavedSudoPasswordAvailable(hasPassword)
      })
      .catch(() => {})

    return () => {
      cancelled = true
    }
  }, [editProfile, form.type])

  useEffect(() => {
    if (!editProfile || form.type !== "ssh" || !form.useJumpHost) {
      return
    }

    const passwordJumps = form.jumpHosts.filter(
      (jump) =>
        jump.authMethod === "password" &&
        jump.host.trim() &&
        jump.username.trim() &&
        Number.isInteger(jump.port)
    )
    if (passwordJumps.length === 0) return

    const loadKey = `${editProfile.id}:${passwordJumps.map(getJumpHostPasswordLookupKey).join("|")}`

    // Only load once per profile + jump identity set to avoid overwriting user input on re-renders.
    if (loadedJumpPasswordsForProfile.current === loadKey) {
      return
    }

    let cancelled = false

    Promise.all(
      passwordJumps.map((jump) => {
        const lookupKey = getJumpHostPasswordLookupKey(jump)
        return invoke<boolean>("has_saved_jump_host_password", {
          profileId: editProfile.id,
          profileName: editProfile.name,
          host: jump.host,
          port: jump.port,
          username: jump.username,
          allowLegacyFallback: form.jumpHosts.length === 1,
        }).then((hasPassword) => ({ lookupKey, hasPassword }))
      })
    )
      .then((results) => {
        if (cancelled) return
        loadedJumpPasswordsForProfile.current = loadKey
        const keys = results.filter((item) => item.hasPassword).map((item) => item.lookupKey)
        setSavedJumpPasswordKeys(new Set(keys))
      })
      .catch(() => {})

    return () => {
      cancelled = true
    }
  }, [editProfile, form.type, form.useJumpHost, form.jumpHosts])

  return {
    savedPasswordAvailable,
    typedPasswordAvailable,
    savedSudoPasswordAvailable,
    savedJumpPasswordKeys,
  }
}
