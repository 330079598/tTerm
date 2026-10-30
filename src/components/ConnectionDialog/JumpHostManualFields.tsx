import React from "react"
import { useTranslation } from "react-i18next"

import { AuthMethodPicker } from "@/components/ConnectionDialog/AuthMethodPicker"
import { CertificatePathField } from "@/components/ConnectionDialog/CertificatePathField"
import { PrivateKeyField } from "@/components/ConnectionDialog/PrivateKeyField"
import { getJumpHostErrorKey } from "@/components/ConnectionDialog/jumpHostPayload"
import { Badge } from "@/components/ui/badge"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { PasswordInput } from "@/components/ui/password-input"
import type { JumpHostForm } from "@/components/ConnectionDialog/types"

const SAVED_PASSWORD_MASK = "********"

interface JumpHostManualFieldsProps {
  jump: JumpHostForm
  onChange: (patch: Partial<JumpHostForm>) => void
  /** The hop's password is saved and nothing new was typed over it. */
  savedPasswordMasked: boolean
  errors: Record<string, string>
  onClearError: (jumpId: string, field: string) => void
}

/** A jump host's address and login, typed in by hand. */
export const JumpHostManualFields: React.FC<JumpHostManualFieldsProps> = ({
  jump,
  onChange,
  savedPasswordMasked,
  errors,
  onClearError,
}) => {
  const { t } = useTranslation()
  const error = (field: string) => errors[getJumpHostErrorKey(jump.id, field)]

  return (
    <>
      <div className="grid grid-cols-3 gap-3">
        <div className="col-span-2">
          <Label htmlFor={`jump-host-${jump.id}`} className="mb-1.5 block">
            {t("jumpHost.host")}
          </Label>
          <Input
            id={`jump-host-${jump.id}`}
            value={jump.host}
            onChange={(e) => {
              onClearError(jump.id, "host")
              onChange({ host: e.target.value })
            }}
            placeholder="bastion.example.com"
            aria-invalid={!!error("host")}
            aria-describedby={error("host") ? `jump-host-error-${jump.id}` : undefined}
          />
          {error("host") && (
            <p
              id={`jump-host-error-${jump.id}`}
              className="text-destructive mt-1 text-xs"
              role="alert"
            >
              {error("host")}
            </p>
          )}
        </div>
        <div>
          <Label htmlFor={`jump-port-${jump.id}`} className="mb-1.5 block">
            {t("connection.port")}
          </Label>
          <Input
            id={`jump-port-${jump.id}`}
            type="number"
            min={1}
            max={65535}
            value={jump.port}
            onChange={(e) => {
              onClearError(jump.id, "port")
              onChange({ port: Number(e.target.value) })
            }}
            aria-invalid={!!error("port")}
            aria-describedby={error("port") ? `jump-port-error-${jump.id}` : undefined}
          />
          {error("port") && (
            <p
              id={`jump-port-error-${jump.id}`}
              className="text-destructive mt-1 text-xs"
              role="alert"
            >
              {error("port")}
            </p>
          )}
        </div>
      </div>

      <div>
        <Label htmlFor={`jump-username-${jump.id}`} className="mb-1.5 block">
          {t("connection.username")}
        </Label>
        <Input
          id={`jump-username-${jump.id}`}
          value={jump.username}
          onChange={(e) => {
            onClearError(jump.id, "username")
            onChange({ username: e.target.value })
          }}
          placeholder={t("connection.usernamePlaceholder")}
          aria-invalid={!!error("username")}
          aria-describedby={error("username") ? `jump-username-error-${jump.id}` : undefined}
        />
        {error("username") && (
          <p
            id={`jump-username-error-${jump.id}`}
            className="text-destructive mt-1 text-xs"
            role="alert"
          >
            {error("username")}
          </p>
        )}
      </div>

      <AuthMethodPicker
        value={jump.authMethod}
        onChange={(authMethod) => onChange({ authMethod })}
      />

      {jump.authMethod === "password" && (
        <div>
          <div className="mb-1.5 flex items-center gap-2">
            <Label htmlFor={`jump-password-${jump.id}`}>{t("connection.password")}</Label>
            {savedPasswordMasked && (
              <Badge variant="secondary" className="h-5 px-1.5 text-[10px]">
                {t("connection.savedPasswordBadge")}
              </Badge>
            )}
          </div>
          <PasswordInput
            id={`jump-password-${jump.id}`}
            revealable={!savedPasswordMasked}
            value={savedPasswordMasked ? SAVED_PASSWORD_MASK : jump.password}
            onChange={(e) => {
              const value = e.target.value
              if (savedPasswordMasked && /^\**$/.test(value)) {
                onChange({ password: "" })
                return
              }
              onChange({
                password:
                  savedPasswordMasked && value.startsWith(SAVED_PASSWORD_MASK)
                    ? value.slice(SAVED_PASSWORD_MASK.length)
                    : value,
              })
            }}
            placeholder={
              savedPasswordMasked ? t("connection.savedPasswordPlaceholder") : "password"
            }
          />
          {savedPasswordMasked && (
            <p className="text-muted-foreground mt-1 text-xs">
              {t("connection.savedPasswordHint")}
            </p>
          )}
        </div>
      )}

      {jump.authMethod === "key" && (
        <>
          <PrivateKeyField
            id={`jump-key-path-${jump.id}`}
            value={jump.privateKeyPath}
            onChange={(privateKeyPath) => {
              onClearError(jump.id, "privateKeyPath")
              onChange({ privateKeyPath })
            }}
            passphrase={jump.privateKeyPassphrase}
            error={error("privateKeyPath")}
          />
          <div>
            <Label htmlFor={`jump-key-pass-${jump.id}`} className="mb-1.5 block">
              {t("ssh.privateKeyPassphrase")}
            </Label>
            <PasswordInput
              id={`jump-key-pass-${jump.id}`}
              value={jump.privateKeyPassphrase}
              onChange={(e) => onChange({ privateKeyPassphrase: e.target.value })}
              placeholder={t("connection.passphrasePlaceholder")}
            />
          </div>
          <CertificatePathField
            id={`jump-certificate-path-${jump.id}`}
            value={jump.certificatePath}
            onChange={(certificatePath) => onChange({ certificatePath })}
          />
        </>
      )}

      {jump.authMethod === "agent" && (
        <p className="text-muted-foreground rounded-md border px-3 py-2 text-xs">
          {t("ssh.sshAgentHint")}
        </p>
      )}

      {jump.authMethod === "interactive" && (
        <p className="text-muted-foreground rounded-md border px-3 py-2 text-xs">
          {t("ssh.interactiveHint")}
        </p>
      )}
    </>
  )
}
