import React from "react"
import { useTranslation } from "react-i18next"

import { AuthMethodPicker } from "@/components/ConnectionDialog/AuthMethodPicker"
import { CertificatePathField } from "@/components/ConnectionDialog/CertificatePathField"
import { EncodingField } from "@/components/ConnectionDialog/EncodingField"
import { PrivateKeyField } from "@/components/ConnectionDialog/PrivateKeyField"
import { Badge } from "@/components/ui/badge"
import { Card, CardContent } from "@/components/ui/card"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import { PasswordInput } from "@/components/ui/password-input"
import { Label } from "@/components/ui/label"

import { normalizeKeepalive } from "@/components/ConnectionDialog/connectionDialogUtils"
import {
  ConnectionForm,
  KEEPALIVE_COUNT_RANGE,
  KEEPALIVE_INTERVAL_RANGE,
} from "@/components/ConnectionDialog/types"

const SAVED_PASSWORD_MASK = "********"

interface SshConnectionFieldsProps {
  form: ConnectionForm
  setForm: React.Dispatch<React.SetStateAction<ConnectionForm>>
  savedPasswordAvailable: boolean
  /** The available password is the one typed while connecting, not yet saved. */
  savedPasswordTyped?: boolean
  matchingGroups: string[]
  nameError: string | null
  setNameError: React.Dispatch<React.SetStateAction<string | null>>
  showGroupDropdown: boolean
  setShowGroupDropdown: React.Dispatch<React.SetStateAction<boolean>>
  titlePlaceholder: string
}

export const SshConnectionFields: React.FC<SshConnectionFieldsProps> = ({
  form,
  setForm,
  savedPasswordAvailable,
  savedPasswordTyped = false,
  matchingGroups,
  nameError,
  setNameError,
  showGroupDropdown,
  setShowGroupDropdown,
  titlePlaceholder,
}) => {
  const { t } = useTranslation()
  const savedPasswordMasked =
    savedPasswordAvailable && form.rememberPassword && form.password.length === 0
  const canRememberPasswords =
    form.authMethod === "password" ||
    (form.useJumpHost && form.jumpHosts.some((jump) => jump.authMethod === "password"))
  const keepalive = normalizeKeepalive(form)

  return (
    <>
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
          placeholder={titlePlaceholder}
        />
        {nameError && <p className="text-destructive mt-1 text-xs">{nameError}</p>}
      </div>

      <div className="relative">
        <Label htmlFor="conn-group" className="mb-1.5 block">
          {t("connection.group")}
        </Label>
        <Input
          id="conn-group"
          value={form.group}
          onChange={(e) => {
            setForm((current) => ({ ...current, group: e.target.value }))
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
              {matchingGroups.map((group) => (
                <button
                  key={group}
                  type="button"
                  className="hover:bg-muted w-full rounded-sm px-3 py-1.5 text-left text-sm"
                  onMouseDown={() => {
                    setForm((current) => ({ ...current, group }))
                    setShowGroupDropdown(false)
                  }}
                >
                  {group}
                </button>
              ))}
            </CardContent>
          </Card>
        )}
      </div>

      <div className="grid grid-cols-3 gap-3">
        <div className="col-span-2">
          <Label htmlFor="conn-host" className="mb-1.5 block">
            {t("connection.host")}
          </Label>
          <Input
            id="conn-host"
            value={form.host}
            onChange={(e) => setForm((current) => ({ ...current, host: e.target.value }))}
            placeholder={t("connection.hostPlaceholder")}
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
            onChange={(e) => setForm((current) => ({ ...current, port: Number(e.target.value) }))}
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
          onChange={(e) => setForm((current) => ({ ...current, username: e.target.value }))}
          placeholder={t("connection.usernamePlaceholder")}
        />
      </div>

      <AuthMethodPicker
        value={form.authMethod}
        onChange={(authMethod) => setForm((current) => ({ ...current, authMethod }))}
      />

      {form.authMethod === "password" && (
        <>
          <div>
            <div className="mb-1.5 flex items-center gap-2">
              <Label htmlFor="conn-password">{t("connection.password")}</Label>
              {savedPasswordMasked && (
                <Badge variant="secondary" className="h-5 px-1.5 text-[10px]">
                  {savedPasswordTyped
                    ? t("quickConnect.typedPasswordBadge")
                    : t("connection.savedPasswordBadge")}
                </Badge>
              )}
            </div>
            <PasswordInput
              id="conn-password"
              revealable={!savedPasswordMasked}
              value={savedPasswordMasked ? SAVED_PASSWORD_MASK : form.password}
              onChange={(e) => {
                const value = e.target.value
                if (savedPasswordMasked && /^\**$/.test(value)) {
                  setForm((current) => ({
                    ...current,
                    password: "",
                    rememberPassword: false,
                  }))
                  return
                }

                const nextValue =
                  savedPasswordMasked && value.startsWith(SAVED_PASSWORD_MASK)
                    ? value.slice(SAVED_PASSWORD_MASK.length)
                    : value
                setForm((current) => ({ ...current, password: nextValue }))
              }}
              placeholder={
                savedPasswordAvailable ? t("connection.savedPasswordPlaceholder") : "password"
              }
            />
            {savedPasswordMasked && (
              <p className="text-muted-foreground mt-1 text-xs">
                {savedPasswordTyped
                  ? t("quickConnect.typedPasswordHint")
                  : t("connection.savedPasswordHint")}
              </p>
            )}
          </div>
        </>
      )}

      {form.authMethod === "key" && (
        <>
          <PrivateKeyField
            id="conn-key-path"
            value={form.privateKeyPath}
            onChange={(privateKeyPath) => setForm((current) => ({ ...current, privateKeyPath }))}
            passphrase={form.privateKeyPassphrase}
          />
          <div>
            <Label htmlFor="conn-key-pass" className="mb-1.5 block">
              {t("ssh.privateKeyPassphrase")}
            </Label>
            <PasswordInput
              id="conn-key-pass"
              value={form.privateKeyPassphrase}
              onChange={(e) =>
                setForm((current) => ({ ...current, privateKeyPassphrase: e.target.value }))
              }
              placeholder={t("connection.passphrasePlaceholder")}
            />
          </div>
          <CertificatePathField
            id="conn-certificate-path"
            value={form.certificatePath}
            onChange={(certificatePath) => setForm((current) => ({ ...current, certificatePath }))}
          />
        </>
      )}

      {form.authMethod === "agent" && (
        <p className="text-muted-foreground rounded-md border px-3 py-2 text-xs">
          {t("ssh.sshAgentHint")}
        </p>
      )}

      {form.authMethod === "interactive" && (
        <p className="text-muted-foreground rounded-md border px-3 py-2 text-xs">
          {t("ssh.interactiveHint")}
        </p>
      )}

      <div className="flex items-start gap-2 rounded-md border px-3 py-2">
        <Checkbox
          id="conn-agent-forward"
          checked={form.agentForward}
          onCheckedChange={(checked) =>
            setForm((current) => ({ ...current, agentForward: checked }))
          }
        />
        <div className="space-y-0.5">
          <Label htmlFor="conn-agent-forward" className="text-sm font-normal">
            {t("ssh.agentForward")}
          </Label>
          <p className="text-muted-foreground text-xs">{t("ssh.agentForwardDesc")}</p>
        </div>
      </div>

      {canRememberPasswords && (
        <div className="flex items-start gap-2 rounded-md border px-3 py-2">
          <Checkbox
            id="conn-remember-password"
            checked={form.rememberPassword}
            onCheckedChange={(checked) =>
              setForm((current) => ({ ...current, rememberPassword: checked }))
            }
          />
          <div className="space-y-0.5">
            <Label htmlFor="conn-remember-password" className="text-sm font-normal">
              {t("connection.rememberCredentials")}
            </Label>
            <p className="text-muted-foreground text-xs">
              {t("connection.rememberCredentialsDesc")}
            </p>
          </div>
        </div>
      )}

      <EncodingField form={form} setForm={setForm} description={t("connection.encodingDesc")} />

      <div>
        <div className="grid grid-cols-2 gap-3">
          <div>
            <Label htmlFor="conn-keepalive-interval" className="mb-1.5 block">
              {t("connection.keepaliveInterval")}
            </Label>
            <Input
              id="conn-keepalive-interval"
              type="number"
              min={KEEPALIVE_INTERVAL_RANGE.min}
              max={KEEPALIVE_INTERVAL_RANGE.max}
              value={form.keepaliveIntervalSecs || ""}
              onChange={(e) =>
                setForm((current) => ({
                  ...current,
                  keepaliveIntervalSecs: Number(e.target.value),
                }))
              }
              onBlur={() =>
                setForm((current) => ({
                  ...current,
                  keepaliveIntervalSecs: keepalive.intervalSecs,
                }))
              }
            />
          </div>
          <div>
            <Label htmlFor="conn-keepalive-count" className="mb-1.5 block">
              {t("connection.keepaliveCountMax")}
            </Label>
            <Input
              id="conn-keepalive-count"
              type="number"
              min={KEEPALIVE_COUNT_RANGE.min}
              max={KEEPALIVE_COUNT_RANGE.max}
              value={form.keepaliveCountMax || ""}
              onChange={(e) =>
                setForm((current) => ({ ...current, keepaliveCountMax: Number(e.target.value) }))
              }
              onBlur={() =>
                setForm((current) => ({ ...current, keepaliveCountMax: keepalive.countMax }))
              }
            />
          </div>
        </div>
        <p className="text-muted-foreground mt-1 text-xs">
          {t("connection.keepaliveDesc", {
            interval: keepalive.intervalSecs,
            count: keepalive.countMax,
            timeout: keepalive.intervalSecs * keepalive.countMax,
          })}
        </p>
      </div>
    </>
  )
}
