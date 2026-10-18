import type { TFunction } from "i18next"
import React from "react"

import {
  buildJumpHostsPayload,
  getJumpHostFieldElementId,
  getJumpHostValidationError,
  getJumpHostValidationErrors,
} from "@/components/ConnectionDialog/jumpHostPayload"
import type { useToast } from "@/hooks/use-toast"
import {
  type ConnectionDialogContentProps,
  type ConnectionForm,
  defaultForm,
} from "@/components/ConnectionDialog/types"
import { SavedProfile } from "@/components/ProfilesPanel"
import { invokeSafe, reportError } from "@/lib/errors"
import { proxyChoicePayload } from "@/lib/proxy"
import { Tab } from "@/types/tab"
import {
  certificatePayload,
  encodingPayload,
  getDefaultTitle,
  loginScriptPayload,
  normalizeKeepalive,
  sessionLogPayload,
} from "@/components/ConnectionDialog/connectionDialogUtils"
import { findActiveTunnelsUsingProfile } from "@/components/TunnelsPanel/profileTunnels"
import { summarizeTunnelNames } from "@/components/TunnelsPanel/tunnelUtils"

interface UseConnectionSubmitOptions {
  form: ConnectionForm
  setForm: React.Dispatch<React.SetStateAction<ConnectionForm>>
  setNameError: React.Dispatch<React.SetStateAction<string | null>>
  setJumpHostErrors: React.Dispatch<React.SetStateAction<Record<string, string>>>
  toast: ReturnType<typeof useToast>["toast"]
  t: TFunction
  editProfile: ConnectionDialogContentProps["editProfile"]
  sshProfileId: string
  savedPasswordAvailable: boolean
  savedJumpPasswordKeys: Set<string>
  allProfiles: SavedProfile[]
  typedPasswordTabId: ConnectionDialogContentProps["typedPasswordTabId"]
  saveOnly: boolean
  onSaved: ConnectionDialogContentProps["onSaved"]
  onClose: ConnectionDialogContentProps["onClose"]
  onConnect: ConnectionDialogContentProps["onConnect"]
  saveConfig: ConnectionDialogContentProps["saveConfig"]
}

/**
 * Submitting the form: saving the profile, connecting, or both, after
 * checking the jump hosts and that the name is not taken.
 */
export function useConnectionSubmit({
  form,
  setForm,
  setNameError,
  setJumpHostErrors,
  toast,
  t,
  editProfile,
  sshProfileId,
  savedPasswordAvailable,
  savedJumpPasswordKeys,
  allProfiles,
  typedPasswordTabId,
  saveOnly,
  onSaved,
  onClose,
  onConnect,
  saveConfig,
}: UseConnectionSubmitOptions) {
  const handleSubmit = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    const keepalive = normalizeKeepalive(form)
    const submitter = (event.nativeEvent as SubmitEvent).submitter as HTMLButtonElement | null
    const submitAction = submitter?.dataset.action
    const shouldReconnect = submitAction === "save-reconnect"
    const shouldSave = submitAction === "save" || shouldReconnect
    setNameError(null)

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

    // An unnamed local tab is named after its shell when it opens.
    const title =
      form.title.trim() || (form.type === "terminal" ? "" : getDefaultTitle(form.type, form))
    const group = form.group.trim()
    const shouldPersistProfile = shouldSave
    const profileId =
      form.type === "ssh" && (shouldPersistProfile || editProfile) ? sshProfileId : undefined
    const ignoreSavedPassword =
      form.type === "ssh" &&
      form.authMethod === "password" &&
      !form.rememberPassword &&
      savedPasswordAvailable &&
      form.password.length === 0
    const profileJumpHostsPayload = buildJumpHostsPayload(form, "snake", undefined, true)
    const connectionJumpHostsPayload = buildJumpHostsPayload(form, "camel", savedJumpPasswordKeys)

    if (shouldPersistProfile && form.type === "ssh" && form.host.trim()) {
      const duplicate = allProfiles.find(
        (profile) => profile.id !== profileId && profile.name === title && profile.group === group
      )
      if (duplicate) {
        setNameError(t("profiles.duplicateName"))
        return
      }

      const profile: SavedProfile = {
        id: sshProfileId,
        name: title,
        group,
        connection_type: form.type,
        host: form.host,
        port: form.port,
        username: form.username,
        password:
          form.authMethod === "password" && form.rememberPassword && form.password.length > 0
            ? form.password
            : undefined,
        ignore_saved_password: ignoreSavedPassword,
        remember_password: form.rememberPassword,
        auth_method: form.authMethod,
        agent_forward: form.agentForward,
        private_key_path: form.authMethod === "key" ? form.privateKeyPath : undefined,
        certificate_path: certificatePayload(form.authMethod, form.certificatePath),
        keepalive_interval_secs: keepalive.intervalSecs,
        keepalive_count_max: keepalive.countMax,
        server_monitor_visible: editProfile?.server_monitor_visible === true,
        encoding: encodingPayload(form.encoding),
        terminal_theme: form.terminalTheme || undefined,
        tab_icon: form.tabIcon,
        // What the backend keeps too, so the open tabs this profile updates
        // go on showing the distribution while the host stays the same.
        distro:
          editProfile?.host === form.host && editProfile.port === form.port
            ? editProfile.distro
            : undefined,
        login_script: loginScriptPayload(form.loginScript),
        shell_integration: form.shellIntegration,
        session_log: sessionLogPayload(form.sessionLog),
        use_jump_host: form.useJumpHost,
        jump_hosts: profileJumpHostsPayload,
        proxy: proxyChoicePayload(form.proxy),
        sudo_autofill: form.sudoAutofill,
        sudo_password:
          form.sudoAutofill && form.sudoPassword.length > 0 ? form.sudoPassword : undefined,
        clear_sudo_password: form.clearSudoPassword,
      }
      const result = await invokeSafe<void>(
        "save_profile",
        { profile, typedPasswordTabId },
        {
          context: "save_profile",
          title: t("errors.profileSaveFailed"),
        }
      )
      if (!result.ok) {
        return
      }
      if (editProfile) {
        const running = await findActiveTunnelsUsingProfile(profile.id)
        if (running.length > 0) {
          toast({
            title: t("tunnels.profileEditedTitle", { defaultValue: "Running tunnels not updated" }),
            description: t("tunnels.profileEditedDescription", {
              names: summarizeTunnelNames(running),
              defaultValue:
                "{{names}} keep the previous connection settings until you restart them.",
            }),
          })
        }
      }
      if (saveOnly) {
        onSaved?.(profile, { reconnect: shouldReconnect })
        onClose()
        return
      }
    }

    // Nothing was saved (no host yet); a save-only dialog never connects.
    if (saveOnly) return

    const connection: Omit<Tab, "id" | "isActive"> = {
      title,
      type: form.type,
      isModified: false,
    }

    if (form.type === "ssh") {
      connection.iconStyle = form.tabIcon
      connection.connection = {
        type: form.type,
        profileId,
        profileName: title,
        host: form.host,
        port: form.port,
        username: form.username,
        password:
          form.authMethod === "password" && form.password.length > 0 ? form.password : undefined,
        ignoreSavedPassword,
        rememberPassword: form.rememberPassword,
        authMethod: form.authMethod,
        agentForward: form.agentForward,
        privateKeyPath: form.authMethod === "key" ? form.privateKeyPath : undefined,
        privateKeyPassphrase: form.authMethod === "key" ? form.privateKeyPassphrase : undefined,
        certificatePath: certificatePayload(form.authMethod, form.certificatePath),
        keepaliveIntervalSecs: keepalive.intervalSecs,
        keepaliveCountMax: keepalive.countMax,
        serverMonitorVisible: editProfile?.server_monitor_visible === true,
        encoding: encodingPayload(form.encoding),
        terminalTheme: form.terminalTheme || undefined,
        loginScript: loginScriptPayload(form.loginScript),
        shellIntegration: form.shellIntegration,
        sessionLog: sessionLogPayload(form.sessionLog),
        jumpHosts: connectionJumpHostsPayload,
        proxy: proxyChoicePayload(form.proxy),
      }
    } else {
      connection.connection = {
        type: "terminal",
        terminalShell: form.terminalShell,
        terminalShellCustomPath:
          form.terminalShell === "custom" ? form.terminalShellCustomPath.trim() : undefined,
        terminalShellCustomArgs:
          form.terminalShell === "custom" ? form.terminalShellCustomArgs.trim() : undefined,
      }

      if (shouldSave) {
        try {
          await saveConfig({
            terminal_shell: form.terminalShell,
            terminal_shell_custom_path:
              form.terminalShell === "custom" ? form.terminalShellCustomPath.trim() : "",
            terminal_shell_custom_args:
              form.terminalShell === "custom" ? form.terminalShellCustomArgs.trim() : "",
          })
        } catch (error) {
          reportError(error, {
            context: "save_config",
            title: t("errors.configSaveFailed"),
          })
          return
        }
      }
    }

    onConnect(connection)
    setForm(defaultForm)
    onClose()
  }

  return {
    handleSubmit,
  }
}
