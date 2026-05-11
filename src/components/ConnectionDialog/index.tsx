import React, { useEffect, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { Loader2, Save, Server, Terminal } from "lucide-react"
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
import { cn } from "@/lib/utils"
import { Tab } from "@/types/tab"

import {
  buildInitialForm,
  getDefaultTitle,
} from "@/components/ConnectionDialog/connectionDialogUtils"
import { JumpHostFields } from "@/components/ConnectionDialog/JumpHostFields"
import { SshConnectionFields } from "@/components/ConnectionDialog/SshConnectionFields"
import { TerminalConnectionFields } from "@/components/ConnectionDialog/TerminalConnectionFields"
import { HostKeyPromptDialog } from "@/components/TerminalTab/HostKeyPromptDialog"
import { HostKeyPromptState } from "@/components/TerminalTab/types"
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

const normalizeJumpAuthMethod = (value: string | undefined): "password" | "key" =>
  value === "key" ? "key" : "password"

const hasJumpHost = (form: ConnectionForm) => form.useJumpHost

const getJumpHostValidationError = (form: ConnectionForm): string | null => {
  if (!hasJumpHost(form)) {
    return null
  }

  if (!form.jumpHost.trim()) {
    return "Jump host is required."
  }

  if (!form.jumpUsername.trim()) {
    return "Jump host username is required."
  }

  if (!Number.isInteger(form.jumpPort) || form.jumpPort < 1 || form.jumpPort > 65535) {
    return "Jump host port must be between 1 and 65535."
  }

  if (form.jumpAuthMethod === "key" && !form.jumpPrivateKeyPath.trim()) {
    return "Jump host private key path is required."
  }

  return null
}

const buildJumpHostPayload = (form: ConnectionForm) => {
  if (!hasJumpHost(form)) {
    return undefined
  }

  const authMethod = normalizeJumpAuthMethod(form.jumpAuthMethod)

  return {
    host: form.jumpHost.trim(),
    port: form.jumpPort,
    username: form.jumpUsername.trim(),
    authMethod,
    password: authMethod === "password" ? form.jumpPassword || undefined : undefined,
    privateKeyPath: authMethod === "key" ? form.jumpPrivateKeyPath || undefined : undefined,
    privateKeyPassphrase:
      authMethod === "key" ? form.jumpPrivateKeyPassphrase || undefined : undefined,
  }
}

const ConnectionDialogContent: React.FC<ConnectionDialogContentProps> = ({
  onClose,
  onConnect,
  editProfile,
  config,
  saveConfig,
}) => {
  const { t } = useTranslation()
  const { toast } = useToast()
  const [form, setForm] = useState<ConnectionForm>(() => buildInitialForm(editProfile, config))
  const [draftProfileId, setDraftProfileId] = useState(() => editProfile?.id ?? crypto.randomUUID())
  const [existingGroups, setExistingGroups] = useState<string[]>([])
  const [showGroupDropdown, setShowGroupDropdown] = useState(false)
  const [nameError, setNameError] = useState<string | null>(null)
  const [allProfiles, setAllProfiles] = useState<SavedProfile[]>([])
  const [isTesting, setIsTesting] = useState(false)
  const [testHostKeyPrompt, setTestHostKeyPrompt] = useState<HostKeyPromptState | null>(null)
  const matchingGroups = existingGroups.filter((group) =>
    group.toLowerCase().includes(form.group.toLowerCase())
  )
  const sshProfileId = editProfile?.id ?? draftProfileId

  useEffect(() => {
    invoke<SavedProfile[]>("list_profiles")
      .then((profiles) => {
        setAllProfiles(profiles)
        const groups = [...new Set(profiles.map((profile) => profile.group).filter(Boolean))]
        setExistingGroups(groups)
      })
      .catch(() => {})
  }, [])

  useEffect(() => {
    setForm(buildInitialForm(editProfile, config))
  }, [editProfile, config])

  useEffect(() => {
    setDraftProfileId(editProfile?.id ?? crypto.randomUUID())
  }, [editProfile])

  useEffect(() => {
    if (!editProfile || form.type !== "ssh" || form.authMethod !== "password") {
      return
    }

    let cancelled = false

    invoke<string | null>("get_saved_password", {
      profileId: editProfile.id,
      profileName: editProfile.name,
    })
      .then((password) => {
        if (cancelled || !password) {
          return
        }

        setForm((current) => {
          if (current.type !== "ssh" || current.authMethod !== "password") {
            return current
          }

          return {
            ...current,
            password,
            rememberPassword: true,
          }
        })
      })
      .catch(() => {})

    return () => {
      cancelled = true
    }
  }, [editProfile, form.authMethod, form.type])

  // Load saved jump host password when editing a profile
  useEffect(() => {
    if (
      !editProfile ||
      form.type !== "ssh" ||
      !form.useJumpHost ||
      form.jumpAuthMethod !== "password"
    ) {
      return
    }

    let cancelled = false

    invoke<string | null>("get_saved_jump_host_password", {
      profileId: editProfile.id,
      profileName: editProfile.name,
    })
      .then((password) => {
        if (cancelled || !password) {
          return
        }

        setForm((current) => {
          if (!current.useJumpHost || current.jumpAuthMethod !== "password") {
            return current
          }

          return {
            ...current,
            jumpPassword: password,
          }
        })
      })
      .catch(() => {})

    return () => {
      cancelled = true
    }
  }, [editProfile, form.type, form.useJumpHost, form.jumpAuthMethod])

  const handleSubmit = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    const submitter = (event.nativeEvent as SubmitEvent).submitter as HTMLButtonElement | null
    const shouldSave = submitter?.dataset.action === "save"
    setNameError(null)

    const jumpValidationError = getJumpHostValidationError(form)
    if (jumpValidationError) {
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
    const profileId = form.type === "ssh" && shouldPersistProfile ? sshProfileId : undefined

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
        remember_password: false,
        auth_method: form.authMethod,
        private_key_path: form.authMethod === "key" ? form.privateKeyPath : undefined,
        keepalive_interval_secs: form.keepaliveIntervalSecs,
        keepalive_count_max: form.keepaliveCountMax,
        jump_host: buildJumpHostPayload(form),
      }
      try {
        await invoke("save_profile", { profile })
      } catch (error) {
        console.error("Failed to save profile:", error)
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
        password: form.authMethod === "password" ? form.password : undefined,
        rememberPassword: form.authMethod === "password" ? form.rememberPassword : undefined,
        privateKeyPath: form.authMethod === "key" ? form.privateKeyPath : undefined,
        privateKeyPassphrase: form.authMethod === "key" ? form.privateKeyPassphrase : undefined,
        keepaliveIntervalSecs: form.keepaliveIntervalSecs,
        keepaliveCountMax: form.keepaliveCountMax,
        jumpHost: buildJumpHostPayload(form),
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
          console.error("Failed to save terminal shell defaults:", error)
        }
      }
    }

    onConnect(connection)
    setForm(defaultForm)
    onClose()
  }

  const handleTestConnection = async () => {
    if (form.type !== "ssh") return

    const jumpValidationError = getJumpHostValidationError(form)
    if (jumpValidationError) {
      toast({
        title: t("profiles.testFailed"),
        description: jumpValidationError,
        variant: "destructive",
      })
      return
    }

    setIsTesting(true)
    const testTabId = `test-${sshProfileId}`
    const unlistenHostPrompt = await listen<HostKeyPromptState>(
      `ssh-hostkey-prompt-${testTabId}`,
      (event) => setTestHostKeyPrompt(event.payload)
    )
    try {
      const title = form.title.trim() || getDefaultTitle(form.type, form)
      const profile: SavedProfile = {
        id: sshProfileId,
        name: title,
        group: form.group.trim(),
        connection_type: form.type,
        host: form.host,
        port: form.port,
        username: form.username,
        password: form.authMethod === "password" ? form.password : undefined,
        auth_method: form.authMethod,
        private_key_path: form.authMethod === "key" ? form.privateKeyPath : undefined,
        private_key_passphrase: form.authMethod === "key" ? form.privateKeyPassphrase : undefined,
        keepalive_interval_secs: form.keepaliveIntervalSecs,
        keepalive_count_max: form.keepaliveCountMax,
        remember_password: false,
        jump_host: buildJumpHostPayload(form),
      }

      const result = await invoke<string>("test_connection", { profile })
      toast({
        title: t("profiles.testSuccess"),
        description: result,
        variant: "success",
        duration: 1000,
      })
    } catch (error) {
      toast({
        title: t("profiles.testFailed"),
        description: String(error),
        variant: "destructive",
      })
    } finally {
      unlistenHostPrompt()
      setTestHostKeyPrompt(null)
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
        className="flex max-h-[85vh] flex-col overflow-hidden sm:max-w-md"
        onInteractOutside={(event) => event.preventDefault()}
      >
        <DialogHeader>
          <DialogTitle>
            {editProfile ? t("profiles.editTitle") : t("connection.newConnection")}
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
                matchingGroups={matchingGroups}
                nameError={nameError}
                setNameError={setNameError}
                showGroupDropdown={showGroupDropdown}
                setShowGroupDropdown={setShowGroupDropdown}
                titlePlaceholder={getDefaultTitle(form.type, form)}
              />
              <JumpHostFields form={form} setForm={setForm} />
            </>
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
}) => {
  const { config, saveConfig } = useConfig()
  const dialogKey = [
    editProfile?.id ?? "new",
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
          config={config}
          saveConfig={saveConfig}
        />
      ) : null}
    </Dialog>
  )
}
