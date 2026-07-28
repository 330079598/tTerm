import React, { useEffect, useRef, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { CheckCircle2, CircleAlert, Loader2, Save, Server, Terminal } from "lucide-react"
import { useTranslation } from "react-i18next"

import { SavedProfile } from "@/components/ProfilesPanel"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Separator } from "@/components/ui/separator"
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip"
import { useConfig } from "@/contexts/ConfigContext"
import { useToast } from "@/hooks/use-toast"
import { invokeSafe, reportError } from "@/lib/errors"
import { cn } from "@/lib/utils"
import { Tab, type JumpHostConnection, type SavedJumpHost } from "@/types/tab"

import {
  buildInitialForm,
  getDefaultTitle,
} from "@/components/ConnectionDialog/connectionDialogUtils"
import { JumpHostFields } from "@/components/ConnectionDialog/JumpHostFields"
import { SshConnectionFields } from "@/components/ConnectionDialog/SshConnectionFields"
import { TerminalConnectionFields } from "@/components/ConnectionDialog/TerminalConnectionFields"
import { HostKeyPromptDialog } from "@/components/TerminalTab/HostKeyPromptDialog"
import { getSshConnectionProgressLabel } from "@/components/TerminalTab/terminalTabUtils"
import { HostKeyPromptState, SshConnectionProgress } from "@/components/TerminalTab/types"
import {
  ConnectionDialogContentProps,
  ConnectionDialogProps,
  ConnectionForm,
  connectionTypes,
  defaultForm,
} from "@/components/ConnectionDialog/types"

const connectionTypeIcons = {
  terminal: Terminal,
  ssh: Server,
} as const

type TestConnectionResultState = {
  status: "success" | "error"
  message: string
}

const normalizeJumpAuthMethod = (value: string | undefined): "password" | "key" =>
  value === "key" ? "key" : "password"

const getJumpHostPasswordLookupKey = (
  jump: Pick<JumpHostConnection, "host" | "port" | "username">
) => `${jump.host.trim()}:${jump.port}:${jump.username.trim()}`

const getJumpHostErrorKey = (jumpId: string, field: string) => `${jumpId}:${field}`

const getJumpHostFieldElementId = (jumpId: string, field: string) => {
  const fieldIds: Record<string, string> = {
    host: "jump-host",
    port: "jump-port",
    username: "jump-username",
    privateKeyPath: "jump-key-path",
  }
  return `${fieldIds[field]}-${jumpId}`
}

const getJumpHostValidationErrors = (form: ConnectionForm): Record<string, string> => {
  const errors: Record<string, string> = {}
  if (!form.useJumpHost) return errors

  for (const [index, jump] of form.jumpHosts.entries()) {
    const label = `Jump host #${index + 1}`
    if (!jump.host.trim())
      errors[getJumpHostErrorKey(jump.id, "host")] = `${label} host is required.`
    if (!jump.username.trim()) {
      errors[getJumpHostErrorKey(jump.id, "username")] = `${label} username is required.`
    }
    if (!Number.isInteger(jump.port) || jump.port < 1 || jump.port > 65535) {
      errors[getJumpHostErrorKey(jump.id, "port")] = `${label} port must be between 1 and 65535.`
    }
    if (jump.authMethod === "key" && !jump.privateKeyPath.trim()) {
      errors[getJumpHostErrorKey(jump.id, "privateKeyPath")] =
        `${label} private key path is required.`
    }
  }
  return errors
}

const getJumpHostValidationError = (form: ConnectionForm): string | null => {
  if (!form.useJumpHost) {
    return null
  }

  if (form.jumpHosts.length === 0) return "At least one jump host is required."

  return Object.values(getJumpHostValidationErrors(form))[0] ?? null
}

function buildJumpHostsPayload(
  form: ConnectionForm,
  keyCase: "snake",
  savedJumpPasswordKeys?: Set<string>,
  preserveWhenDisabled?: boolean
): SavedJumpHost[] | undefined
function buildJumpHostsPayload(
  form: ConnectionForm,
  keyCase: "camel",
  savedJumpPasswordKeys?: Set<string>
): JumpHostConnection[] | undefined
function buildJumpHostsPayload(
  form: ConnectionForm,
  keyCase: "camel" | "snake",
  savedJumpPasswordKeys: Set<string> = new Set(),
  preserveWhenDisabled = false
): SavedJumpHost[] | JumpHostConnection[] | undefined {
  if (form.jumpHosts.length === 0 || (!form.useJumpHost && !preserveWhenDisabled)) {
    return undefined
  }

  if (keyCase === "snake") {
    return form.jumpHosts.map((jump) => {
      const authMethod = normalizeJumpAuthMethod(jump.authMethod)
      const privateKeyPath = authMethod === "key" ? jump.privateKeyPath || undefined : undefined
      const privateKeyPassphrase =
        authMethod === "key" ? jump.privateKeyPassphrase || undefined : undefined
      return {
        host: jump.host.trim(),
        port: jump.port,
        username: jump.username.trim(),
        auth_method: authMethod,
        password: authMethod === "password" ? jump.password || undefined : undefined,
        private_key_path: privateKeyPath,
        private_key_passphrase: privateKeyPassphrase,
      }
    })
  }

  return form.jumpHosts.map((jump) => {
    const authMethod = normalizeJumpAuthMethod(jump.authMethod)
    const savedPasswordAvailable = savedJumpPasswordKeys.has(getJumpHostPasswordLookupKey(jump))
    return {
      host: jump.host.trim(),
      port: jump.port,
      username: jump.username.trim(),
      password:
        authMethod === "password" && (!savedPasswordAvailable || jump.password.length > 0)
          ? jump.password || undefined
          : undefined,
      authMethod,
      privateKeyPath: authMethod === "key" ? jump.privateKeyPath || undefined : undefined,
      privateKeyPassphrase:
        authMethod === "key" ? jump.privateKeyPassphrase || undefined : undefined,
    }
  })
}

const ConnectionDialogContent: React.FC<ConnectionDialogContentProps> = ({
  onClose,
  onConnect,
  editProfile,
  duplicateProfile,
  config,
  saveConfig,
}) => {
  const { t } = useTranslation()
  const { toast } = useToast()
  const [form, setForm] = useState<ConnectionForm>(() => {
    const initialForm = buildInitialForm(editProfile ?? duplicateProfile, config)
    if (duplicateProfile) {
      initialForm.title = t("profiles.copyName", { name: duplicateProfile.name })
    }
    return initialForm
  })
  const [draftProfileId, setDraftProfileId] = useState(() => editProfile?.id ?? crypto.randomUUID())
  const [existingGroups, setExistingGroups] = useState<string[]>([])
  const [showGroupDropdown, setShowGroupDropdown] = useState(false)
  const [nameError, setNameError] = useState<string | null>(null)
  const [allProfiles, setAllProfiles] = useState<SavedProfile[]>([])
  const [isTesting, setIsTesting] = useState(false)
  const [testProgress, setTestProgress] = useState<SshConnectionProgress | null>(null)
  const [testResult, setTestResult] = useState<TestConnectionResultState | null>(null)
  const [testHostKeyPrompt, setTestHostKeyPrompt] = useState<HostKeyPromptState | null>(null)
  const [savedPasswordAvailable, setSavedPasswordAvailable] = useState(false)
  const [savedJumpPasswordKeys, setSavedJumpPasswordKeys] = useState<Set<string>>(() => new Set())
  const [jumpHostErrors, setJumpHostErrors] = useState<Record<string, string>>({})
  const loadedJumpPasswordsForProfile = useRef<string | null>(null)
  const matchingGroups = existingGroups.filter((group) =>
    group.toLowerCase().includes(form.group.toLowerCase())
  )
  const sshProfileId = editProfile?.id ?? draftProfileId

  useEffect(() => {
    Promise.all([invoke<SavedProfile[]>("list_profiles"), invoke<string[]>("list_profile_groups")])
      .then(([profiles, configuredGroups]) => {
        setAllProfiles(profiles)
        const groups = [
          ...new Set([
            ...configuredGroups,
            ...profiles.map((profile) => profile.group).filter(Boolean),
          ]),
        ].sort((left, right) => left.localeCompare(right))
        setExistingGroups(groups)
      })
      .catch(() => {})
  }, [])

  useEffect(() => {
    loadedJumpPasswordsForProfile.current = null
    setSavedPasswordAvailable(false)
    setSavedJumpPasswordKeys(new Set())
    const initialForm = buildInitialForm(editProfile ?? duplicateProfile, config)
    if (duplicateProfile) {
      initialForm.title = t("profiles.copyName", { name: duplicateProfile.name })
    }
    setForm(initialForm)
  }, [editProfile, duplicateProfile, config, t])

  useEffect(() => {
    setDraftProfileId(editProfile?.id ?? crypto.randomUUID())
  }, [editProfile])

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
  }, [editProfile, form.authMethod, form.type])

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

  const handleSubmit = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    const submitter = (event.nativeEvent as SubmitEvent).submitter as HTMLButtonElement | null
    const shouldSave = submitter?.dataset.action === "save"
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

    const title = form.title.trim() || getDefaultTitle(form.type, form)
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
        private_key_path: form.authMethod === "key" ? form.privateKeyPath : undefined,
        keepalive_interval_secs: form.keepaliveIntervalSecs,
        keepalive_count_max: form.keepaliveCountMax,
        server_monitor_visible: editProfile?.server_monitor_visible === true,
        use_jump_host: form.useJumpHost,
        jump_hosts: profileJumpHostsPayload,
      }
      const result = await invokeSafe<void>(
        "save_profile",
        { profile },
        {
          context: "save_profile",
          title: t("errors.profileSaveFailed"),
        }
      )
      if (!result.ok) {
        return
      }
    }

    const connection: Omit<Tab, "id" | "isActive"> = {
      title,
      type: form.type,
      isModified: false,
    }

    if (form.type === "ssh") {
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
        privateKeyPath: form.authMethod === "key" ? form.privateKeyPath : undefined,
        privateKeyPassphrase: form.authMethod === "key" ? form.privateKeyPassphrase : undefined,
        keepaliveIntervalSecs: form.keepaliveIntervalSecs,
        keepaliveCountMax: form.keepaliveCountMax,
        serverMonitorVisible: editProfile?.server_monitor_visible === true,
        jumpHosts: connectionJumpHostsPayload,
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

  const handleTestConnection = async () => {
    if (form.type !== "ssh") return

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
        keepalive_interval_secs: form.keepaliveIntervalSecs,
        keepalive_count_max: form.keepaliveCountMax,
        server_monitor_visible: editProfile?.server_monitor_visible === true,
        remember_password: false,
        use_jump_host: form.useJumpHost,
        jump_hosts: jumpHostsPayload,
      }

      const result = await invoke<{ message: string; networkLatencyMs: number | null }>(
        "test_connection",
        { profile }
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

  const isSsh = form.type === "ssh"
  const isTerminal = form.type === "terminal"

  return (
    <>
      <DialogContent
        className="flex max-h-[85vh] flex-col overflow-hidden sm:max-w-[600px]"
        onInteractOutside={(event) => event.preventDefault()}
      >
        <DialogHeader>
          <DialogTitle>
            {editProfile
              ? t("profiles.editTitle")
              : duplicateProfile
                ? t("profiles.copyTitle")
                : t("connection.newConnection")}
          </DialogTitle>
        </DialogHeader>

        <form onSubmit={handleSubmit} className="flex-1 space-y-5 overflow-y-auto px-1">
          <div>
            <Label className="mb-2 block">{t("connection.type")}</Label>
            <div className="grid grid-cols-2 gap-2">
              {connectionTypes.map(({ type, label }) => {
                const Icon = connectionTypeIcons[type]

                return (
                  <Button
                    key={type}
                    type="button"
                    variant={form.type === type ? "default" : "outline"}
                    onClick={() => setForm((current) => ({ ...current, type }))}
                    className={cn(
                      "h-auto justify-start gap-2 px-3 py-2.5 text-sm transition-colors",
                      form.type === type ? "shadow-none" : "text-muted-foreground"
                    )}
                  >
                    <Icon size={16} />
                    {label}
                  </Button>
                )
              })}
            </div>
          </div>

          <Separator />

          {!isSsh && (
            <div>
              <Label htmlFor="conn-title" className="mb-1.5 block">
                {t("connection.title")}
              </Label>
              <Input
                id="conn-title"
                value={form.title}
                onChange={(e) => {
                  setForm((current) => ({ ...current, title: e.target.value }))
                  setNameError(null)
                }}
                placeholder={getDefaultTitle(form.type, form)}
              />
              {nameError && <p className="text-destructive mt-1 text-xs">{nameError}</p>}
            </div>
          )}

          {isTerminal && <TerminalConnectionFields form={form} setForm={setForm} />}

          {isSsh && (
            <>
              <SshConnectionFields
                form={form}
                setForm={setForm}
                savedPasswordAvailable={savedPasswordAvailable}
                matchingGroups={matchingGroups}
                nameError={nameError}
                setNameError={setNameError}
                showGroupDropdown={showGroupDropdown}
                setShowGroupDropdown={setShowGroupDropdown}
                titlePlaceholder={getDefaultTitle(form.type, form)}
              />
              <JumpHostFields
                form={form}
                setForm={setForm}
                savedJumpPasswordKeys={savedJumpPasswordKeys}
                errors={jumpHostErrors}
                onClearError={(jumpId, field) =>
                  setJumpHostErrors((current) => {
                    const key = getJumpHostErrorKey(jumpId, field)
                    if (!current[key]) return current
                    const { [key]: _, ...remaining } = current
                    return remaining
                  })
                }
              />
            </>
          )}

          {isSsh && testProgress && (
            <div
              role="status"
              aria-live="polite"
              className="border-border bg-muted/35 text-muted-foreground rounded-md border px-3 py-2 text-xs"
            >
              {getSshConnectionProgressLabel(testProgress)}
            </div>
          )}

          {isSsh && !testProgress && testResult && (
            <div
              role={testResult.status === "error" ? "alert" : "status"}
              aria-live="polite"
              className={cn(
                "flex items-start gap-2 rounded-md border px-3 py-2 text-xs",
                testResult.status === "success"
                  ? "border-green-500/40 bg-green-500/10 text-green-700 dark:text-green-300"
                  : "border-destructive/40 bg-destructive/10 text-destructive"
              )}
            >
              {testResult.status === "success" ? (
                <CheckCircle2 className="mt-0.5 size-4 shrink-0" />
              ) : (
                <CircleAlert className="mt-0.5 size-4 shrink-0" />
              )}
              <span>
                <span className="font-medium">
                  {testResult.status === "success"
                    ? t("profiles.testSuccess")
                    : t("profiles.testFailed")}
                  :{" "}
                </span>
                {testResult.message}
              </span>
            </div>
          )}

          <DialogFooter>
            <Button type="button" variant="ghost" onClick={onClose}>
              {t("connection.cancel")}
            </Button>
            {isSsh && (
              <Button
                key={isTesting ? "testing" : "test"}
                type="button"
                variant="outline"
                onClick={handleTestConnection}
                disabled={isTesting || !form.host.trim() || !form.username.trim()}
                className="min-w-[100px]"
              >
                {isTesting && <Loader2 className="animate-spin" />}
                {isTesting ? t("profiles.testing") : t("profiles.test")}
              </Button>
            )}
            <TooltipProvider>
              {isSsh && (
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button type="submit" variant="outline" data-action="save">
                      <Save size={14} />
                      {t("connection.saveAndConnect")}
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>{t("connection.saveAndConnectDescription")}</TooltipContent>
                </Tooltip>
              )}
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button type="submit" data-action="connect">
                    {t("connection.connect")}
                  </Button>
                </TooltipTrigger>
                <TooltipContent>{t("connection.connectDescription")}</TooltipContent>
              </Tooltip>
            </TooltipProvider>
          </DialogFooter>
        </form>
      </DialogContent>
      <HostKeyPromptDialog
        hostKeyPrompt={testHostKeyPrompt}
        setHostKeyPrompt={setTestHostKeyPrompt}
      />
    </>
  )
}

export const ConnectionDialog: React.FC<ConnectionDialogProps> = ({
  isOpen,
  onClose,
  onConnect,
  editProfile,
  duplicateProfile,
}) => {
  const { config, saveConfig } = useConfig()
  const dialogKey = [
    editProfile?.id ?? duplicateProfile?.id ?? "new",
    duplicateProfile ? "duplicate" : "edit",
    config.terminal_shell,
    config.terminal_shell_custom_path,
    config.terminal_shell_custom_args,
  ].join("::")

  return (
    <Dialog
      open={isOpen}
      onOpenChange={(open) => {
        if (!open) onClose()
      }}
    >
      {isOpen ? (
        <ConnectionDialogContent
          key={dialogKey}
          onClose={onClose}
          onConnect={onConnect}
          editProfile={editProfile}
          duplicateProfile={duplicateProfile}
          config={config}
          saveConfig={saveConfig}
        />
      ) : null}
    </Dialog>
  )
}
