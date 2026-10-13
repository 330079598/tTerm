import React from "react"
import { useTranslation } from "react-i18next"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { PasswordInput } from "@/components/ui/password-input"
import type { ProxyDraftError } from "@/lib/proxy"

interface ProxyServerFieldsProps {
  host: string
  portText: string
  username: string
  password: string
  /** Whether a password is saved for the server. */
  savedPassword: boolean
  /** Whether the saved password is to be removed on save. */
  clearPassword: boolean
  fieldError: (field: ProxyDraftError) => React.ReactNode
  onField: (
    field: "host" | "portText" | "username"
  ) => (event: React.ChangeEvent<HTMLInputElement>) => void
  onPassword: (password: string) => void
  onClearPassword: () => void
}

/** The address and sign-in fields of an HTTP or SOCKS5 proxy server. */
export const ProxyServerFields: React.FC<ProxyServerFieldsProps> = ({
  host,
  portText,
  username,
  password,
  savedPassword,
  clearPassword,
  fieldError,
  onField,
  onPassword,
  onClearPassword,
}) => {
  const { t } = useTranslation()
  const hasSavedPassword = savedPassword && !clearPassword

  return (
    <>
      <div className="grid grid-cols-[1fr_7rem] gap-3">
        <div>
          <Label htmlFor="proxy-host" className="mb-1.5 block">
            {t("proxies.form.host")}
          </Label>
          <Input id="proxy-host" value={host} onChange={onField("host")} />
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
            onChange={onField("portText")}
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
            onChange={onField("username")}
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
            onChange={(event) => onPassword(event.target.value)}
          />
        </div>
      </div>
      {savedPassword && (
        <Button
          type="button"
          variant="outline"
          size="sm"
          disabled={clearPassword}
          onClick={onClearPassword}
        >
          {t("proxies.form.clearPassword")}
        </Button>
      )}
    </>
  )
}
