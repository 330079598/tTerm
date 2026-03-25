import "@/components/ConnectionDialog.css"
import React, { useState } from "react"
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

interface ConnectionDialogProps {
  isOpen: boolean
  onClose: () => void
  onConnect: (connection: Omit<Tab, "id" | "isActive">) => void
}

type ConnectionType = "terminal" | "ssh" | "sftp" | "serial"

interface ConnectionForm {
  type: ConnectionType
  title: string
  host: string
  port: number
  username: string
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
  host: "",
  port: 22,
  username: "",
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
}) => {
  const { t } = useTranslation()
  const [form, setForm] = useState<ConnectionForm>(defaultForm)

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
        host: form.host,
        port: form.port,
        username: form.username,
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

  return (
    <Dialog open={isOpen} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t("connection.newConnection")}</DialogTitle>
        </DialogHeader>

        <form onSubmit={handleSubmit} className="space-y-4">
          {/* Connection type selector */}
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

          {/* Title */}
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

          {/* SSH/SFTP fields */}
          {form.type !== "terminal" && form.type !== "serial" && (
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

          <DialogFooter>
            <Button type="button" variant="ghost" onClick={onClose}>
              {t("connection.cancel")}
            </Button>
            <Button type="submit">{t("connection.connect")}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
