import "@/components/ConnectionDialog.css"
import React, { useState, useEffect } from "react"
import { useTranslation } from "react-i18next"
import { Terminal, Server, FolderOpen, Zap } from "lucide-react"
import { Tab } from "@/types/tab"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Separator } from "@/components/ui/separator"
import { cn } from "@/lib/utils"
import { open as openFileDialog } from "@tauri-apps/plugin-dialog"
import { invoke } from "@tauri-apps/api/core"
import { SavedProfile } from "@/components/ProfilesPanel"

interface ConnectionDialogProps {
  isOpen: boolean
  onClose: () => void
  onConnect: (connection: Omit<Tab, "id" | "isActive">) => void
  editProfile?: SavedProfile | null
}

type ConnectionType = "terminal" | "ssh" | "sftp" | "serial"

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
  reconnect: true,
  reconnectDelaySecs: 3,
  reconnectMaxDelaySecs: 60,
  reconnectMaxRetries: 8,
  keepaliveIntervalSecs: 15,
  keepaliveCountMax: 3,
}

const connectionTypes = [
  { type: "terminal" as const, label: "Local Terminal", icon: Terminal },
  { type: "ssh" as const, label: "SSH Connection", icon: Server },
  { type: "sftp" as const, label: "SFTP Browser", icon: FolderOpen },
  { type: "serial" as const, label: "Serial Port", icon: Zap },
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

function getDefaultTitle(type: ConnectionType, form: ConnectionForm): string {
  switch (type) {
    case "terminal":
      return "Local Terminal"
    case "ssh":
      return form.host ? `${form.username}@${form.host}` : "SSH Connection"
    case "sftp":
      return form.host ? `SFTP: ${form.host}` : "SFTP Browser"
    case "serial":
      return "Serial Port"
  }
}

export const ConnectionDialog: React.FC<ConnectionDialogProps> = ({
  isOpen,
  onClose,
  onConnect,
  editProfile,
}) => {
  const { t } = useTranslation()
  const [form, setForm] = useState<ConnectionForm>(() => buildFormFromProfile(editProfile))
  const [saveStatus, setSaveStatus] = useState<"idle" | "saving" | "saved">("idle")
  const [existingGroups, setExistingGroups] = useState<string[]>([])
  const [showGroupDropdown, setShowGroupDropdown] = useState(false)

  // Load existing groups for autocomplete
  useEffect(() => {
    if (isOpen) {
      invoke<SavedProfile[]>("list_profiles")
        .then((profiles) => {
          const groups = [...new Set(profiles.map((p) => p.group).filter(Boolean))]
          setExistingGroups(groups)
        })
        .catch(() => {})
    }
  }, [isOpen])

  const handleSaveProfile = async () => {
    if (form.type !== "ssh" || !form.host.trim()) return
    const profile: SavedProfile = {
      id: editProfile?.id ?? crypto.randomUUID(),
      name: form.title.trim() || `${form.username}@${form.host}`,
      group: form.group.trim(),
      connection_type: form.type,
      host: form.host,
      port: form.port,
      username: form.username,
      auth_method: form.authMethod,
      private_key_path: form.authMethod === "key" ? form.privateKeyPath : undefined,
      reconnect: form.reconnect,
      reconnect_delay_secs: form.reconnectDelaySecs,
      reconnect_max_delay_secs: form.reconnectMaxDelaySecs,
      reconnect_max_retries: form.reconnectMaxRetries,
      keepalive_interval_secs: form.keepaliveIntervalSecs,
      keepalive_count_max: form.keepaliveCountMax,
    }
    setSaveStatus("saving")
    try {
      await invoke("save_profile", { profile })
      setSaveStatus("saved")
      setTimeout(() => setSaveStatus("idle"), 2000)
    } catch (e) {
      console.error("Failed to save profile:", e)
      setSaveStatus("idle")
    }
  }

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()

    const connection: Omit<Tab, "id" | "isActive"> = {
      title: form.title || getDefaultTitle(form.type, form),
      type: form.type,
      isModified: false,
    }

    if (form.type !== "terminal") {
      connection.connection = {
        type: form.type,
        profileName: form.type === "ssh" ? form.title.trim() || undefined : undefined,
        host: form.host,
        port: form.port,
        username: form.username,
        password: form.type === "ssh" && form.authMethod === "password" ? form.password : undefined,
        rememberPassword:
          form.type === "ssh" && form.authMethod === "password" ? form.rememberPassword : undefined,
        privateKeyPath:
          form.type === "ssh" && form.authMethod === "key" ? form.privateKeyPath : undefined,
        privateKeyPassphrase:
          form.type === "ssh" && form.authMethod === "key" ? form.privateKeyPassphrase : undefined,
        reconnect: form.reconnect,
        reconnectDelaySecs: form.reconnectDelaySecs,
        reconnectMaxDelaySecs: form.reconnectMaxDelaySecs,
        reconnectMaxRetries: form.reconnectMaxRetries,
        keepaliveIntervalSecs: form.keepaliveIntervalSecs,
        keepaliveCountMax: form.keepaliveCountMax,
      }
    }

    if (form.type === "terminal") {
      connection.connection = {
        type: "terminal",
      }
    }

    onConnect(connection)
    setForm(defaultForm)
    onClose()
  }

  const isSsh = form.type === "ssh"
  const isRemote = form.type !== "terminal" && form.type !== "serial"

  return (
    <Dialog open={isOpen} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>
            {editProfile ? t("profiles.editTitle") : t("connection.newConnection")}
          </DialogTitle>
        </DialogHeader>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <Label className="mb-2 block">{t("connection.type")}</Label>
            <div className="grid grid-cols-2 gap-2">
              {connectionTypes.map(({ type, label, icon: Icon }) => (
                <button
                  key={type}
                  type="button"
                  onClick={() => setForm((f) => ({ ...f, type }))}
                  className={cn(
                    "flex items-center gap-2 rounded-md border px-3 py-2.5 text-sm transition-colors",
                    "hover:bg-muted",
                    form.type === type
                      ? "border-primary bg-primary/10 text-primary"
                      : "border-border text-muted-foreground bg-transparent"
                  )}
                >
                  <Icon size={16} />
                  {label}
                </button>
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
              onChange={(e) => setForm((f) => ({ ...f, title: e.target.value }))}
              placeholder={getDefaultTitle(form.type, form)}
            />
          </div>

          {isSsh && (
            <div style={{ position: "relative" }}>
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
              {showGroupDropdown &&
                existingGroups.filter((g) => g.toLowerCase().includes(form.group.toLowerCase()))
                  .length > 0 && (
                  <div
                    style={{
                      position: "absolute",
                      top: "100%",
                      left: 0,
                      right: 0,
                      zIndex: 20,
                      background: "hsl(var(--popover))",
                      border: "1px solid hsl(var(--border))",
                      borderRadius: 6,
                      marginTop: 2,
                      maxHeight: 160,
                      overflowY: "auto",
                    }}
                  >
                    {existingGroups
                      .filter((g) => g.toLowerCase().includes(form.group.toLowerCase()))
                      .map((g) => (
                        <div
                          key={g}
                          style={{ padding: "6px 12px", cursor: "pointer", fontSize: 13 }}
                          className="hover:bg-muted"
                          onMouseDown={() => {
                            setForm((f) => ({ ...f, group: g }))
                            setShowGroupDropdown(false)
                          }}
                        >
                          {g}
                        </div>
                      ))}
                  </div>
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
                  <button
                    type="button"
                    className={cn(
                      "flex-1 rounded border px-3 py-1.5 text-sm",
                      form.authMethod === "password"
                        ? "border-primary bg-primary text-primary-foreground"
                        : "border-border bg-background text-foreground"
                    )}
                    onClick={() => setForm((f) => ({ ...f, authMethod: "password" }))}
                  >
                    {t("ssh.password")}
                  </button>
                  <button
                    type="button"
                    className={cn(
                      "flex-1 rounded border px-3 py-1.5 text-sm",
                      form.authMethod === "key"
                        ? "border-primary bg-primary text-primary-foreground"
                        : "border-border bg-background text-foreground"
                    )}
                    onClick={() => setForm((f) => ({ ...f, authMethod: "key" }))}
                  >
                    {t("ssh.sshKey")}
                  </button>
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
                  <label className="text-foreground flex items-center gap-2 text-sm">
                    <input
                      type="checkbox"
                      checked={form.rememberPassword}
                      onChange={(e) =>
                        setForm((f) => ({ ...f, rememberPassword: e.target.checked }))
                      }
                    />
                    <span>{t("connection.rememberPassword")}</span>
                  </label>
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
                            filters: [{ name: "All Files", extensions: ["*"] }],
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
                type="button"
                variant="outline"
                onClick={handleSaveProfile}
                disabled={saveStatus === "saving" || !form.host.trim()}
              >
                {saveStatus === "saved" ? t("profiles.saveSuccess") : t("profiles.save")}
              </Button>
            )}
            <Button type="submit">{t("connection.connect")}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
