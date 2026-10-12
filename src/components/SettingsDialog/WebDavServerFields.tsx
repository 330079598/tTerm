import { useTranslation } from "react-i18next"
import React from "react"
import { AlertTriangle, Eye, EyeOff } from "lucide-react"

import type {
  WebDavBackupSettings,
  WebDavBackupStatus,
} from "@/components/SettingsDialog/webdavBackup"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"

interface WebDavServerFieldsProps {
  form: WebDavBackupSettings
  status: WebDavBackupStatus
  busy: boolean
  update: (patch: Partial<WebDavBackupSettings>) => void
  password: string
  setPassword: (value: string) => void
  passwordType: "text" | "password"
  savedPlaceholder: string
  showPasswords: boolean
  setShowPasswords: React.Dispatch<React.SetStateAction<boolean>>
  location: string | null
}

/** Where backups go: the WebDAV server, its account and the folder on it. */
export function WebDavServerFields({
  form,
  status,
  busy,
  update,
  password,
  setPassword,
  passwordType,
  savedPlaceholder,
  showPasswords,
  setShowPasswords,
  location,
}: WebDavServerFieldsProps) {
  const { t } = useTranslation()

  return (
    <>
      <div>
        <Label htmlFor="webdav-url">{t("dataMigration.webdav.url")}</Label>
        <Input
          id="webdav-url"
          className="mt-1.5"
          value={form.url}
          disabled={busy}
          placeholder="https://dav.jianguoyun.com/dav/"
          spellCheck={false}
          autoCapitalize="off"
          onChange={(event) => update({ url: event.target.value })}
        />
        <p className="text-muted-foreground mt-1 text-xs">{t("dataMigration.webdav.urlHint")}</p>
      </div>

      {form.url.trim().toLowerCase().startsWith("http://") && (
        <Alert className="border-amber-500/40 bg-amber-500/10">
          <AlertTriangle size={16} className="absolute top-3.5 left-4" />
          <AlertDescription className="pl-6">
            {t("dataMigration.webdav.insecureUrl")}
          </AlertDescription>
        </Alert>
      )}

      <div className="grid gap-3 sm:grid-cols-2">
        <div>
          <Label htmlFor="webdav-username">{t("dataMigration.webdav.username")}</Label>
          <Input
            id="webdav-username"
            className="mt-1.5"
            value={form.username}
            disabled={busy}
            spellCheck={false}
            autoCapitalize="off"
            onChange={(event) => update({ username: event.target.value })}
          />
        </div>
        <div>
          <Label htmlFor="webdav-password">{t("dataMigration.webdav.password")}</Label>
          <div className="relative mt-1.5">
            <Input
              id="webdav-password"
              type={passwordType}
              value={password}
              disabled={busy}
              placeholder={status.hasPassword ? savedPlaceholder : ""}
              onChange={(event) => setPassword(event.target.value)}
              className="pr-10"
            />
            <Button
              type="button"
              variant="ghost"
              size="icon"
              className="absolute top-0 right-0"
              onClick={() => setShowPasswords((visible) => !visible)}
              aria-label={t("dataMigration.togglePassword")}
            >
              {showPasswords ? <EyeOff size={16} /> : <Eye size={16} />}
            </Button>
          </div>
        </div>
      </div>

      <div>
        <Label htmlFor="webdav-directory">{t("dataMigration.webdav.remoteDirectory")}</Label>
        <Input
          id="webdav-directory"
          className="mt-1.5"
          value={form.remoteDirectory}
          disabled={busy}
          spellCheck={false}
          onChange={(event) => update({ remoteDirectory: event.target.value })}
        />
        {location && (
          <p className="text-muted-foreground mt-1 text-xs break-all">
            {t("dataMigration.webdav.locationPreview", { location })}
          </p>
        )}
      </div>
    </>
  )
}
