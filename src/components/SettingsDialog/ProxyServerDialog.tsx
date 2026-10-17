import React, { useState } from "react"
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
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import { Spinner } from "@/components/ui/spinner"
import { ProxyServerFields } from "@/components/SettingsDialog/ProxyServerFields"
import type { ProxyPasswordChange } from "@/hooks/useProxyServers"
import { DEFAULT_PROXY_PORTS, validateProxyDraft, type ProxyDraftError } from "@/lib/proxy"
import { toErrorMessage } from "@/lib/utils"
import type { NetworkProxyKind, ProxyKind, ProxyServer, ProxyServerEntry } from "@/types/proxy"

interface ProxyServerDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** The server being edited, or null to add one. */
  server: ProxyServerEntry | null
  onSave: (server: ProxyServer, change: ProxyPasswordChange) => Promise<void>
}

type ProxyServerFormProps = Omit<ProxyServerDialogProps, "open">

/** The form's text fields, as typed. */
interface DraftFields {
  name: string
  kind: ProxyKind
  host: string
  portText: string
  username: string
  command: string
}

function initialFields(server: ProxyServerEntry | null): DraftFields {
  const isServer = server && server.kind !== "command"
  return {
    name: server?.name ?? "",
    kind: server?.kind ?? "socks5",
    host: isServer ? server.host : "127.0.0.1",
    portText: String(isServer ? server.port : DEFAULT_PROXY_PORTS.socks5),
    username: server?.username ?? "",
    command: server?.command ?? "",
  }
}

/**
 * The port after switching to the kind `next`: an untouched usual port
 * follows the kind, a typed one stays.
 */
function followPort(fields: DraftFields, next: ProxyKind): string {
  if (fields.kind === "command" || next === "command") return fields.portText
  return fields.portText.trim() === String(DEFAULT_PROXY_PORTS[fields.kind])
    ? String(DEFAULT_PROXY_PORTS[next as NetworkProxyKind])
    : fields.portText
}

const ProxyServerForm: React.FC<ProxyServerFormProps> = ({ onOpenChange, server, onSave }) => {
  const { t } = useTranslation()
  const [fields, setFields] = useState(() => initialFields(server))
  const [password, setPassword] = useState("")
  const [clearPassword, setClearPassword] = useState(false)
  const [error, setError] = useState<ProxyDraftError | null>(null)
  const [saveError, setSaveError] = useState<string | null>(null)
  const [saving, setSaving] = useState(false)

  const { name, kind, host, portText, username, command } = fields
  const isCommand = kind === "command"
  const port = /^\d+$/.test(portText.trim()) ? Number(portText.trim()) : 0
  const setField = (field: keyof DraftFields) => (event: React.ChangeEvent<HTMLInputElement>) =>
    setFields((current) => ({ ...current, [field]: event.target.value }))

  const changeKind = (next: ProxyKind) => {
    setFields((current) => ({ ...current, kind: next, portText: followPort(current, next) }))
  }

  const handleSave = async () => {
    const draft: ProxyServer = isCommand
      ? {
          id: server?.id ?? crypto.randomUUID(),
          name: name.trim(),
          kind,
          host: "",
          port: 0,
          command: command.trim(),
        }
      : {
          id: server?.id ?? crypto.randomUUID(),
          name: name.trim(),
          kind,
          host: host.trim(),
          port,
          username: username.trim() || undefined,
        }
    const invalid = validateProxyDraft(draft)
    setError(invalid)
    if (invalid) {
      document.getElementById(`proxy-${invalid}`)?.focus()
      return
    }
    setSaving(true)
    setSaveError(null)
    try {
      await onSave(draft, isCommand ? { clearPassword: true } : { password, clearPassword })
      onOpenChange(false)
    } catch (err) {
      setSaveError(toErrorMessage(err))
      setSaving(false)
    }
  }

  const fieldError = (field: ProxyDraftError) =>
    error === field ? (
      <p className="text-destructive mt-1 text-xs">{t(`proxies.errors.${field}`)}</p>
    ) : null

  return (
    <form
      onSubmit={(event) => {
        event.preventDefault()
        void handleSave()
      }}
    >
      <DialogHeader className="border-border/80 border-b px-6 py-5">
        <DialogTitle>{server ? t("proxies.editTitle") : t("proxies.newTitle")}</DialogTitle>
        <DialogDescription>{t("proxies.dialogDescription")}</DialogDescription>
      </DialogHeader>

      <div className="space-y-4 px-6 py-5">
        <div>
          <Label htmlFor="proxy-name" className="mb-1.5 block">
            {t("proxies.form.name")}
          </Label>
          <Input
            id="proxy-name"
            value={name}
            placeholder={t("proxies.form.namePlaceholder")}
            onChange={setField("name")}
          />
          {fieldError("name")}
        </div>

        <div>
          <Label htmlFor="proxy-kind" className="mb-1.5 block">
            {t("proxies.form.kind")}
          </Label>
          <Select
            id="proxy-kind"
            value={kind}
            onChange={(event) => changeKind(event.target.value as ProxyKind)}
          >
            <option value="socks5">{t("proxies.kinds.socks5")}</option>
            <option value="http">{t("proxies.kinds.http")}</option>
            <option value="command">{t("proxies.kinds.command")}</option>
          </Select>
        </div>

        {isCommand ? (
          <div>
            <Label htmlFor="proxy-command" className="mb-1.5 block">
              {t("proxies.form.command")}
            </Label>
            <Input
              id="proxy-command"
              className="font-mono"
              autoComplete="off"
              spellCheck={false}
              value={command}
              placeholder="ssh -W %h:%p bastion"
              onChange={setField("command")}
            />
            {fieldError("command")}
            {fieldError("commandToken")}
            <p className="text-muted-foreground mt-1.5 text-xs leading-5">
              {t("proxies.form.commandHint")}
            </p>
          </div>
        ) : (
          <ProxyServerFields
            host={host}
            portText={portText}
            username={username}
            password={password}
            savedPassword={server?.hasPassword === true}
            clearPassword={clearPassword}
            fieldError={fieldError}
            onField={setField}
            onPassword={(value) => {
              setPassword(value)
              setClearPassword(false)
            }}
            onClearPassword={() => {
              setPassword("")
              setClearPassword(true)
            }}
          />
        )}

        {saveError && (
          <p className="text-destructive text-sm" role="alert">
            {saveError}
          </p>
        )}
      </div>

      <DialogFooter className="border-border/80 border-t px-6 py-4">
        <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
          {t("common.cancel")}
        </Button>
        <Button type="submit" disabled={saving}>
          {saving && <Spinner />}
          {t("common.save")}
        </Button>
      </DialogFooter>
    </form>
  )
}

export const ProxyServerDialog: React.FC<ProxyServerDialogProps> = ({
  open,
  onOpenChange,
  ...formProps
}) => (
  <Dialog open={open} onOpenChange={onOpenChange}>
    <DialogContent className="gap-0 overflow-hidden p-0 sm:max-w-lg">
      {/* Remounting per open gives every session a fresh draft. */}
      {open && (
        <ProxyServerForm
          key={formProps.server?.id ?? "new"}
          onOpenChange={onOpenChange}
          {...formProps}
        />
      )}
    </DialogContent>
  </Dialog>
)
