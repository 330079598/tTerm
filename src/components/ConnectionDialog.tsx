import React, { useState, useEffect } from "react"
import { useTranslation } from "react-i18next"
import { Terminal, Server, Loader2 } from "lucide-react"
import { Tab, TerminalShellType } from "@/types/tab"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import { Separator } from "@/components/ui/separator"
import { cn } from "@/lib/utils"
import { open as openFileDialog } from "@tauri-apps/plugin-dialog"
import { invoke } from "@tauri-apps/api/core"
import { SavedProfile } from "@/components/ProfilesPanel"
import { useConfig } from "@/contexts/ConfigContext"
import { useToast } from "@/hooks/use-toast"

interface ConnectionDialogProps {
  isOpen: boolean
  onClose: () => void
  onConnect: (connection: Omit<Tab, "id" | "isActive">) => void
  editProfile?: SavedProfile | null
}

type ConnectionType = "terminal" | "ssh"

type ConfigState = ReturnType<typeof useConfig>["config"]
type SaveConfig = ReturnType<typeof useConfig>["saveConfig"]

interface ConnectionForm {
  type: ConnectionType
  title: string
  group: string
  host: string
  port: number
  username: string
  authMethod: "password" | "key"
  password: string
  rememberPassword: boolean
  privateKeyPath: string
  privateKeyPassphrase: string
  reconnect: boolean
  reconnectDelaySecs: number
  reconnectMaxDelaySecs: number
  reconnectMaxRetries: number
  keepaliveIntervalSecs: number
  keepaliveCountMax: number
  terminalShell: TerminalShellType
  terminalShellCustomPath: string
  terminalShellCustomArgs: string
}

interface ConnectionDialogContentProps extends Omit<ConnectionDialogProps, "isOpen"> {
  config: ConfigState
  saveConfig: SaveConfig
}

const defaultForm: ConnectionForm = {
  type: "terminal",
  title: "",
  group: "",
  host: "",
  port: 22,
  username: "",
  authMethod: "password",
  password: "",
  rememberPassword: false,
  privateKeyPath: "",
  privateKeyPassphrase: "",
  reconnect: false,
  reconnectDelaySecs: 3,
  reconnectMaxDelaySecs: 60,
  reconnectMaxRetries: 8,
  keepaliveIntervalSecs: 15,
  keepaliveCountMax: 3,
  terminalShell: "auto",
  terminalShellCustomPath: "",
  terminalShellCustomArgs: "",
}

const connectionTypes = [
  { type: "terminal" as const, label: "OS terminal", icon: Terminal },
  { type: "ssh" as const, label: "SSH Connection", icon: Server },
]

function buildFormFromProfile(profile?: SavedProfile | null): ConnectionForm {
  if (!profile) {
    return { ...defaultForm }
  }

  return {
    ...defaultForm,
    type: profile.connection_type as ConnectionType,
    title: profile.name,
    group: profile.group ?? "",
    host: profile.host ?? "",
    port: profile.port ?? 22,
    username: profile.username ?? "",
    authMethod: (profile.auth_method as "password" | "key") ?? "password",
    privateKeyPath: profile.private_key_path ?? "",
    reconnect: profile.reconnect,
    reconnectDelaySecs: profile.reconnect_delay_secs,
    reconnectMaxDelaySecs: profile.reconnect_max_delay_secs,
    reconnectMaxRetries: profile.reconnect_max_retries,
    keepaliveIntervalSecs: profile.keepalive_interval_secs,
    keepaliveCountMax: profile.keepalive_count_max,
  }
}

function buildInitialForm(
  profile: SavedProfile | null | undefined,
  config: ConfigState
): ConnectionForm {
  const form = buildFormFromProfile(profile)

  if (!profile || form.type === "terminal") {
    form.terminalShell = config.terminal_shell
    form.terminalShellCustomPath = config.terminal_shell_custom_path
    form.terminalShellCustomArgs = config.terminal_shell_custom_args
  }

  return form
}

function getDefaultTitle(type: ConnectionType, form: ConnectionForm): string {
  switch (type) {
    case "terminal":
      return "OS terminal"
    case "ssh":
      return form.host ? `${form.username}@${form.host}` : "SSH Connection"
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
  const [existingGroups, setExistingGroups] = useState<string[]>([])
  const [showGroupDropdown, setShowGroupDropdown] = useState(false)
  const [nameError, setNameError] = useState<string | null>(null)
  const [allProfiles, setAllProfiles] = useState<SavedProfile[]>([])
  const [isTesting, setIsTesting] = useState(false)
  const matchingGroups = existingGroups.filter((g) =>
    g.toLowerCase().includes(form.group.toLowerCase())
  )

  useEffect(() => {
    invoke<SavedProfile[]>("list_profiles")
      .then((profiles) => {
        setAllProfiles(profiles)
        const groups = [...new Set(profiles.map((p) => p.group).filter(Boolean))]
        setExistingGroups(groups)
      })
      .catch(() => {})
  }, [])

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setNameError(null)

    const title = form.title.trim() || getDefaultTitle(form.type, form)
    const group = form.group.trim()

    if (form.type === "ssh" && form.host.trim()) {
      const profileId = editProfile?.id ?? crypto.randomUUID()
      const duplicate = allProfiles.find(
        (p) => p.id !== profileId && p.name === title && p.group === group
      )
      if (duplicate) {
        setNameError(t("profiles.duplicateName"))
        return
      }
      const profile: SavedProfile = {
        id: profileId,
        name: title,
        group,
        connection_type: form.type,
        host: form.host,
        port: form.port,
        username: form.username,
        remember_password: false,
        auth_method: form.authMethod,
        private_key_path: form.authMethod === "key" ? form.privateKeyPath : undefined,
        reconnect: form.reconnect,
        reconnect_delay_secs: form.reconnectDelaySecs,
        reconnect_max_delay_secs: form.reconnectMaxDelaySecs,
        reconnect_max_retries: form.reconnectMaxRetries,
        keepalive_interval_secs: form.keepaliveIntervalSecs,
        keepalive_count_max: form.keepaliveCountMax,
      }
      try {
        await invoke("save_profile", { profile })
      } catch (e) {
        console.error("Failed to auto-save profile:", e)
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
        profileName: title,
        host: form.host,
        port: form.port,
        username: form.username,
        password: form.authMethod === "password" ? form.password : undefined,
        rememberPassword: form.authMethod === "password" ? form.rememberPassword : undefined,
        privateKeyPath: form.authMethod === "key" ? form.privateKeyPath : undefined,
        privateKeyPassphrase: form.authMethod === "key" ? form.privateKeyPassphrase : undefined,
        reconnect: form.reconnect,
        reconnectDelaySecs: form.reconnectDelaySecs,
        reconnectMaxDelaySecs: form.reconnectMaxDelaySecs,
        reconnectMaxRetries: form.reconnectMaxRetries,
        keepaliveIntervalSecs: form.keepaliveIntervalSecs,
        keepaliveCountMax: form.keepaliveCountMax,
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

    onConnect(connection)
    setForm(defaultForm)
    onClose()
  }

  const handleTestConnection = async () => {
    if (form.type !== "ssh") return

    setIsTesting(true)
    try {
      const title = form.title.trim() || getDefaultTitle(form.type, form)
      const profile: SavedProfile = {
        id: crypto.randomUUID(),
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
        reconnect: form.reconnect,
        reconnect_delay_secs: form.reconnectDelaySecs,
        reconnect_max_delay_secs: form.reconnectMaxDelaySecs,
        reconnect_max_retries: form.reconnectMaxRetries,
        keepalive_interval_secs: form.keepaliveIntervalSecs,
        keepalive_count_max: form.keepaliveCountMax,
        remember_password: false,
      }

      const result = await invoke<string>("test_connection", { profile })
      toast({
        title: t("profiles.testSuccess"),
        description: result,
        variant: "success",
        duration: 1000,
      })
    } catch (e) {
      toast({
        title: t("profiles.testFailed"),
        description: String(e),
        variant: "destructive",
      })
    } finally {
      // Small delay to ensure UI updates properly
      await new Promise((resolve) => setTimeout(resolve, 100))
      setIsTesting(false)
    }
  }

  const isSsh = form.type === "ssh"
  const isTerminal = form.type === "terminal"
  const isRemote = form.type !== "terminal"

  return (
    <DialogContent
      className="flex max-h-[85vh] flex-col overflow-hidden sm:max-w-md"
      onInteractOutside={(e) => e.preventDefault()}
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
            {connectionTypes.map(({ type, label, icon: Icon }) => (
              <Button
                key={type}
                type="button"
                variant={form.type === type ? "default" : "outline"}
                onClick={() => setForm((f) => ({ ...f, type }))}
                className={cn(
                  "h-auto justify-start gap-2 px-3 py-2.5 text-sm transition-colors",
                  form.type === type ? "shadow-none" : "text-muted-foreground"
                )}
              >
                <Icon size={16} />
                {label}
              </Button>
            ))}
          </div>
        </div>

        <Separator />

        <div>
          <Label htmlFor="conn-title" className="mb-1.5 block">
            {t("connection.title")}
          </Label>
          <Input
            id="conn-title"
            value={form.title}
            onChange={(e) => {
              setForm((f) => ({ ...f, title: e.target.value }))
              setNameError(null)
            }}
            placeholder={getDefaultTitle(form.type, form)}
          />
          {nameError && <p className="text-destructive mt-1 text-xs">{nameError}</p>}
        </div>

        {isTerminal && (
          <>
            <div>
              <Label htmlFor="conn-terminal-shell" className="mb-1.5 block">
                {t("connection.terminalShell")}
              </Label>
              <Select
                id="conn-terminal-shell"
                value={form.terminalShell}
                onChange={(e) =>
                  setForm((f) => ({
                    ...f,
                    terminalShell: e.target.value as TerminalShellType,
                  }))
                }
              >
                <option value="auto">{t("connection.terminalShellOptions.auto")}</option>
                <option value="cmd">{t("connection.terminalShellOptions.cmd")}</option>
                <option value="powershell">
                  {t("connection.terminalShellOptions.powershell")}
                </option>
                <option value="pwsh">{t("connection.terminalShellOptions.pwsh")}</option>
                <option value="custom">{t("connection.terminalShellOptions.custom")}</option>
              </Select>
            </div>

            {form.terminalShell === "custom" && (
              <>
                <div>
                  <Label htmlFor="conn-terminal-shell-path" className="mb-1.5 block">
                    {t("connection.terminalShellCustomPath")}
                  </Label>
                  <Input
                    id="conn-terminal-shell-path"
                    value={form.terminalShellCustomPath}
                    onChange={(e) =>
                      setForm((f) => ({ ...f, terminalShellCustomPath: e.target.value }))
                    }
                    placeholder="C:\\Program Files\\PowerShell\\7\\pwsh.exe"
                    required
                  />
                </div>
                <div>
                  <Label htmlFor="conn-terminal-shell-args" className="mb-1.5 block">
                    {t("connection.terminalShellCustomArgs")}
                  </Label>
                  <Input
                    id="conn-terminal-shell-args"
                    value={form.terminalShellCustomArgs}
                    onChange={(e) =>
                      setForm((f) => ({ ...f, terminalShellCustomArgs: e.target.value }))
                    }
                    placeholder="-NoLogo"
                  />
                </div>
              </>
            )}
          </>
        )}

        {isSsh && (
          <div className="relative">
            <Label htmlFor="conn-group" className="mb-1.5 block">
              {t("connection.group")}
            </Label>
            <Input
              id="conn-group"
              value={form.group}
              onChange={(e) => {
                setForm((f) => ({ ...f, group: e.target.value }))
                setShowGroupDropdown(true)
              }}
              onFocus={() => setShowGroupDropdown(true)}
              onBlur={() => setTimeout(() => setShowGroupDropdown(false), 150)}
              placeholder={t("connection.groupPlaceholder")}
              autoComplete="off"
            />
            {showGroupDropdown && matchingGroups.length > 0 && (
              <Card className="absolute top-full right-0 left-0 z-20 mt-1 max-h-40 overflow-y-auto rounded-md">
                <CardContent className="p-1">
                  {matchingGroups.map((g) => (
                    <button
                      key={g}
                      type="button"
                      className="hover:bg-muted w-full rounded-sm px-3 py-1.5 text-left text-sm"
                      onMouseDown={() => {
                        setForm((f) => ({ ...f, group: g }))
                        setShowGroupDropdown(false)
                      }}
                    >
                      {g}
                    </button>
                  ))}
                </CardContent>
              </Card>
            )}
          </div>
        )}

        {isRemote && (
          <>
            <div className="grid grid-cols-3 gap-3">
              <div className="col-span-2">
                <Label htmlFor="conn-host" className="mb-1.5 block">
                  {t("connection.host")}
                </Label>
                <Input
                  id="conn-host"
                  value={form.host}
                  onChange={(e) => setForm((f) => ({ ...f, host: e.target.value }))}
                  placeholder="hostname or IP"
                  required
                />
              </div>
              <div>
                <Label htmlFor="conn-port" className="mb-1.5 block">
                  {t("connection.port")}
                </Label>
                <Input
                  id="conn-port"
                  type="number"
                  value={form.port}
                  onChange={(e) => setForm((f) => ({ ...f, port: Number(e.target.value) }))}
                />
              </div>
            </div>
            <div>
              <Label htmlFor="conn-user" className="mb-1.5 block">
                {t("connection.username")}
              </Label>
              <Input
                id="conn-user"
                value={form.username}
                onChange={(e) => setForm((f) => ({ ...f, username: e.target.value }))}
                placeholder="username"
              />
            </div>
          </>
        )}

        {isSsh && (
          <>
            <div>
              <Label className="mb-1.5 block">{t("ssh.authMethod")}</Label>
              <div className="flex gap-2">
                <Button
                  type="button"
                  variant={form.authMethod === "password" ? "default" : "outline"}
                  className={cn(
                    "flex-1",
                    form.authMethod === "password" ? "shadow-none" : "text-muted-foreground"
                  )}
                  onClick={() => setForm((f) => ({ ...f, authMethod: "password" }))}
                >
                  {t("ssh.password")}
                </Button>
                <Button
                  type="button"
                  variant={form.authMethod === "key" ? "default" : "outline"}
                  className={cn(
                    "flex-1",
                    form.authMethod === "key" ? "shadow-none" : "text-muted-foreground"
                  )}
                  onClick={() => setForm((f) => ({ ...f, authMethod: "key" }))}
                >
                  {t("ssh.sshKey")}
                </Button>
              </div>
            </div>

            {form.authMethod === "password" && (
              <>
                <div>
                  <Label htmlFor="conn-password" className="mb-1.5 block">
                    {t("connection.password")}
                  </Label>
                  <Input
                    id="conn-password"
                    type="password"
                    value={form.password}
                    onChange={(e) => setForm((f) => ({ ...f, password: e.target.value }))}
                    placeholder="password"
                  />
                </div>
                <div className="flex items-center gap-2 rounded-md border px-3 py-2">
                  <Checkbox
                    id="conn-remember-password"
                    checked={form.rememberPassword}
                    onCheckedChange={(checked) =>
                      setForm((f) => ({ ...f, rememberPassword: checked }))
                    }
                  />
                  <Label htmlFor="conn-remember-password" className="text-sm font-normal">
                    {t("connection.rememberPassword")}
                  </Label>
                </div>
              </>
            )}

            {form.authMethod === "key" && (
              <>
                <div>
                  <Label htmlFor="conn-key-path" className="mb-1.5 block">
                    {t("ssh.privateKeyPath")}
                  </Label>
                  <div className="flex gap-2">
                    <Input
                      id="conn-key-path"
                      value={form.privateKeyPath}
                      onChange={(e) => setForm((f) => ({ ...f, privateKeyPath: e.target.value }))}
                      placeholder="~/.ssh/id_rsa"
                      className="flex-1"
                    />
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      onClick={async () => {
                        const selected = await openFileDialog({
                          multiple: false,
                        }).catch(() => null)
                        if (selected && typeof selected === "string") {
                          setForm((f) => ({ ...f, privateKeyPath: selected }))
                        }
                      }}
                    >
                      {t("ssh.browseKey")}
                    </Button>
                  </div>
                </div>
                <div>
                  <Label htmlFor="conn-key-pass" className="mb-1.5 block">
                    {t("ssh.privateKeyPassphrase")}
                  </Label>
                  <Input
                    id="conn-key-pass"
                    type="password"
                    value={form.privateKeyPassphrase}
                    onChange={(e) =>
                      setForm((f) => ({ ...f, privateKeyPassphrase: e.target.value }))
                    }
                    placeholder="passphrase"
                  />
                </div>
              </>
            )}
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
          <Button type="submit">{t("connection.connect")}</Button>
        </DialogFooter>
      </form>
    </DialogContent>
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
