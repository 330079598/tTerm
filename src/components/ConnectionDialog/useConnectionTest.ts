import type { TFunction } from "i18next"
import React, { useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"

import {
  buildJumpHostsPayload,
  getJumpHostFieldElementId,
  getJumpHostValidationError,
  getJumpHostValidationErrors,
} from "@/components/ConnectionDialog/jumpHostPayload"
import type { useToast } from "@/hooks/use-toast"
import type { useUserVerification } from "@/contexts/UserVerificationContext"
import type {
  ConnectionDialogContentProps,
  ConnectionForm,
} from "@/components/ConnectionDialog/types"
import { SavedProfile } from "@/components/ProfilesPanel"
import { isVerificationCanceled } from "@/lib/userVerification"
import {
  certificatePayload,
  getDefaultTitle,
  normalizeKeepalive,
} from "@/components/ConnectionDialog/connectionDialogUtils"
import { HostKeyPromptState, SshConnectionProgress } from "@/components/TerminalTab/types"

type TestConnectionResultState = {
  status: "success" | "error"
  message: string
}

interface UseConnectionTestOptions {
  form: ConnectionForm
  setJumpHostErrors: React.Dispatch<React.SetStateAction<Record<string, string>>>
  toast: ReturnType<typeof useToast>["toast"]
  t: TFunction
  sshProfileId: string
  savedPasswordAvailable: boolean
  editProfile: ConnectionDialogContentProps["editProfile"]
  withVaultUnlock: ReturnType<typeof useUserVerification>["withVaultUnlock"]
  typedPasswordTabId: ConnectionDialogContentProps["typedPasswordTabId"]
}

/** Testing the connection as filled in, with its progress and result. */
export function useConnectionTest({
  form,
  setJumpHostErrors,
  toast,
  t,
  sshProfileId,
  savedPasswordAvailable,
  editProfile,
  withVaultUnlock,
  typedPasswordTabId,
}: UseConnectionTestOptions) {
  const [isTesting, setIsTesting] = useState(false)
  const [testProgress, setTestProgress] = useState<SshConnectionProgress | null>(null)
  const [testResult, setTestResult] = useState<TestConnectionResultState | null>(null)
  const [testHostKeyPrompt, setTestHostKeyPrompt] = useState<HostKeyPromptState | null>(null)

  const handleTestConnection = async () => {
    if (form.type !== "ssh") return
    const keepalive = normalizeKeepalive(form)

    const jumpErrors = getJumpHostValidationErrors(form)
    setJumpHostErrors(jumpErrors)
    const jumpValidationError = getJumpHostValidationError(form)
    if (jumpValidationError) {
      const firstErrorKey = Object.keys(jumpErrors)[0]
      if (firstErrorKey) {
        const [jumpId, field] = firstErrorKey.split(":")
        window.setTimeout(() =>
          document.getElementById(getJumpHostFieldElementId(jumpId, field))?.focus()
        )
      }
      toast({
        title: t("profiles.testFailed"),
        description: jumpValidationError,
        variant: "destructive",
      })
      return
    }

    setIsTesting(true)
    setTestResult(null)
    setTestProgress({
      phase: "resolving_credentials",
      message: t("profiles.testing", { defaultValue: "Testing connection..." }),
    })
    const testTabId = `test-${sshProfileId}`
    let unlistenProgress = () => {}
    let unlistenHostPrompt = () => {}
    try {
      unlistenProgress = await listen<SshConnectionProgress>(
        `ssh-connection-progress-${testTabId}`,
        (event) => setTestProgress(event.payload)
      )
      unlistenHostPrompt = await listen<HostKeyPromptState>(
        `ssh-hostkey-prompt-${testTabId}`,
        (event) => setTestHostKeyPrompt(event.payload)
      )

      const title = form.title.trim() || getDefaultTitle(form.type, form)
      const jumpHostsPayload = buildJumpHostsPayload(form, "snake")
      const ignoreSavedPassword =
        form.authMethod === "password" &&
        !form.rememberPassword &&
        savedPasswordAvailable &&
        form.password.length === 0
      const profile: SavedProfile = {
        id: sshProfileId,
        name: title,
        group: form.group.trim(),
        connection_type: form.type,
        host: form.host,
        port: form.port,
        username: form.username,
        password:
          form.authMethod === "password" && form.password.length > 0 ? form.password : undefined,
        ignore_saved_password: ignoreSavedPassword,
        auth_method: form.authMethod,
        private_key_path: form.authMethod === "key" ? form.privateKeyPath : undefined,
        private_key_passphrase: form.authMethod === "key" ? form.privateKeyPassphrase : undefined,
        certificate_path: certificatePayload(form.authMethod, form.certificatePath),
        keepalive_interval_secs: keepalive.intervalSecs,
        keepalive_count_max: keepalive.countMax,
        server_monitor_visible: editProfile?.server_monitor_visible === true,
        remember_password: false,
        use_jump_host: form.useJumpHost,
        jump_hosts: jumpHostsPayload,
      }

      const result = await withVaultUnlock(() =>
        invoke<{ message: string; networkLatencyMs: number | null }>("test_connection", {
          profile,
          typedPasswordTabId,
        })
      )
      const target = `${form.username}@${form.host}:${form.port}`
      const successMessage =
        result.networkLatencyMs == null
          ? t("profiles.testSuccessDescriptionNoLatency", {
              target,
              defaultValue: "Connected to {{target}}. Latency unavailable.",
            })
          : t("profiles.testSuccessDescription", {
              target,
              latency: result.networkLatencyMs,
              defaultValue: "Connected to {{target}} · Latency {{latency}} ms",
            })
      setTestResult({ status: "success", message: successMessage })
      toast({
        title: t("profiles.testSuccess"),
        description: successMessage,
        variant: "success",
        duration: 2500,
      })
    } catch (error) {
      if (isVerificationCanceled(error)) return
      const errorMessage = String(error)
      setTestResult({ status: "error", message: errorMessage })
      toast({
        title: t("profiles.testFailed"),
        description: errorMessage,
        variant: "destructive",
      })
    } finally {
      unlistenProgress()
      unlistenHostPrompt()
      setTestHostKeyPrompt(null)
      setTestProgress(null)
      // Small delay to ensure UI updates properly.
      await new Promise((resolve) => setTimeout(resolve, 100))
      setIsTesting(false)
    }
  }

  return {
    isTesting,
    testProgress,
    testResult,
    testHostKeyPrompt,
    setTestHostKeyPrompt,
    handleTestConnection,
  }
}
