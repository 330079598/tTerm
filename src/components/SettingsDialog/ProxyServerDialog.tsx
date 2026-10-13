import React, { useState } from "react"
import { Loader2 } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Badge } from "@/components/ui/badge"
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
import { PasswordInput } from "@/components/ui/password-input"
import { Select } from "@/components/ui/select"
import type { ProxyPasswordChange } from "@/hooks/useProxyServers"
import { DEFAULT_PROXY_PORTS, validateProxyDraft, type ProxyDraftError } from "@/lib/proxy"
import { toErrorMessage } from "@/lib/utils"
import type { ProxyKind, ProxyServer, ProxyServerEntry } from "@/types/proxy"

interface ProxyServerDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** The server being edited, or null to add one. */
  server: ProxyServerEntry | null
  onSave: (server: ProxyServer, change: ProxyPasswordChange) => Promise<void>
}

type ProxyServerFormProps = Omit<ProxyServerDialogProps, "open">

const ProxyServerForm: React.FC<ProxyServerFormProps> = ({ onOpenChange, server, onSave }) => {
  const { t } = useTranslation()
  const [name, setName] = useState(server?.name ?? "")
  const [kind, setKind] = useState<ProxyKind>(server?.kind ?? "socks5")
  const [host, setHost] = useState(server?.host ?? "127.0.0.1")
  const [portText, setPortText] = useState(String(server?.port ?? DEFAULT_PROXY_PORTS.socks5))
  const [username, setUsername] = useState(server?.username ?? "")
  const [password, setPassword] = useState("")
  const [clearPassword, setClearPassword] = useState(false)
  const [error, setError] = useState<ProxyDraftError | null>(null)
  const [saveError, setSaveError] = useState<string | null>(null)
  const [saving, setSaving] = useState(false)

  const hasSavedPassword = server?.hasPassword === true && !clearPassword
  const port = /^\d+$/.test(portText.trim()) ? Number(portText.trim()) : 0

  const changeKind = (next: ProxyKind) => {
    // Follow the usual port of the new kind unless one was typed.
    if (portText.trim() === String(DEFAULT_PROXY_PORTS[kind])) {
      setPortText(String(DEFAULT_PROXY_PORTS[next]))
    }
    setKind(next)
  }

  const handleSave = async () => {
    const draft: ProxyServer = {
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
      await onSave(draft, { password, clearPassword })
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
            onChange={(event) => setName(event.target.value)}
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
          </Select>
        </div>

        <div className="grid grid-cols-[1fr_7rem] gap-3">
          <div>
            <Label htmlFor="proxy-host" className="mb-1.5 block">
              {t("proxies.form.host")}
            </Label>
            <Input id="proxy-host" value={host} onChange={(event) => setHost(event.target.value)} />
            {fieldError("host")}
          </div>
          <div>
            <Label htmlFor="proxy-port" className="mb-1.5 block">
              {t("proxies.form.port")}
            </Label>
            <Input
              id="proxy-port"
              inputMode="numeric"
              value={portText}
              onChange={(event) => setPortText(event.target.value)}
            />
            {fieldError("port")}
          </div>
        </div>

        <div className="grid grid-cols-2 gap-3">
          <div>
            <Label htmlFor="proxy-username" className="mb-1.5 block">
              {t("proxies.form.username")}
            </Label>
            <Input
              id="proxy-username"
              autoComplete="off"
              value={username}
              placeholder={t("proxies.form.usernamePlaceholder")}
              onChange={(event) => setUsername(event.target.value)}
            />
          </div>
          <div>
            <div className="mb-1.5 flex items-center gap-2">
              <Label htmlFor="proxy-password">{t("proxies.form.password")}</Label>
              {hasSavedPassword && (
                <Badge variant="secondary" className="h-5 px-1.5 text-[10px]">
                  {t("connection.savedPasswordBadge")}
                </Badge>
              )}
            </div>
            <PasswordInput
              id="proxy-password"
              autoComplete="new-password"
              value={password}
              disabled={!username.trim()}
              placeholder={hasSavedPassword ? t("proxies.form.passwordSavedPlaceholder") : ""}
              onChange={(event) => {
                setPassword(event.target.value)
                setClearPassword(false)
              }}
            />
          </div>
        </div>
        {server?.hasPassword && (
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={clearPassword}
            onClick={() => {
              setPassword("")
              setClearPassword(true)
            }}
          >
            {t("proxies.form.clearPassword")}
          </Button>
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
          {saving && <Loader2 className="animate-spin" />}
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
