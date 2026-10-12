import { KeyRound, Trash2 } from "lucide-react"
import { useTranslation } from "react-i18next"

import { ActionIcon, PasswordField } from "@/components/SettingsDialog/SecretFields"
import type { SecretStorageMode } from "@/lib/appConfig"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import type { VaultAction } from "@/components/SettingsDialog/types"

interface MasterPasswordCardProps {
  busy: boolean
  action: VaultAction | null
  mode: SecretStorageMode
  hasMasterPassword: boolean
  currentPassword: string
  newPassword: string
  confirmPassword: string
  setCurrentPassword: (value: string) => void
  setNewPassword: (value: string) => void
  setConfirmPassword: (value: string) => void
  handleChangePassword: () => Promise<void>
  handleSetRecovery: () => Promise<void>
  handleRemoveRecovery: () => Promise<void>
}

/** Changing the master password, or setting and removing the recovery password. */
export function MasterPasswordCard({
  busy,
  action,
  mode,
  hasMasterPassword,
  currentPassword,
  newPassword,
  confirmPassword,
  setCurrentPassword,
  setNewPassword,
  setConfirmPassword,
  handleChangePassword,
  handleSetRecovery,
  handleRemoveRecovery,
}: MasterPasswordCardProps) {
  const { t } = useTranslation()

  return (
    <Card>
      <CardContent className="space-y-3 p-4">
        <div>
          <div className="text-sm font-medium">
            {mode === "system"
              ? t("secretStorage.recoveryTitle")
              : t("secretStorage.changePassword")}
          </div>
          <div className="text-muted-foreground mt-1 text-xs leading-5">
            {mode === "system"
              ? t("secretStorage.recoveryDesc")
              : t("secretStorage.changePasswordDesc")}
          </div>
          {mode === "system" && hasMasterPassword && (
            <div className="mt-1 text-xs leading-5">{t("secretStorage.recoveryIsSet")}</div>
          )}
        </div>
        {hasMasterPassword && (
          <PasswordField
            id="current-password"
            label={t("secretStorage.currentPassword")}
            value={currentPassword}
            onChange={setCurrentPassword}
            disabled={busy}
            autoComplete="current-password"
          />
        )}
        <PasswordField
          id="new-password"
          label={t("secretStorage.newPassword")}
          value={newPassword}
          onChange={setNewPassword}
          disabled={busy}
          autoComplete="new-password"
        />
        <PasswordField
          id="confirm-password"
          label={t("secretStorage.confirmNewPassword")}
          value={confirmPassword}
          onChange={setConfirmPassword}
          disabled={busy}
          autoComplete="new-password"
        />
        <div className="flex flex-wrap gap-2">
          {hasMasterPassword ? (
            <Button
              onClick={() => void handleChangePassword()}
              disabled={
                busy ||
                currentPassword.length === 0 ||
                newPassword.length === 0 ||
                confirmPassword.length === 0
              }
            >
              <ActionIcon
                busy={action === "changePassword"}
                icon={<KeyRound size={14} className="mr-2" />}
              />
              {action === "changePassword"
                ? t("secretStorage.changingPassword")
                : t("secretStorage.changePassword")}
            </Button>
          ) : (
            <Button
              onClick={() => void handleSetRecovery()}
              disabled={busy || newPassword.length === 0 || confirmPassword.length === 0}
            >
              <ActionIcon
                busy={action === "setPassword"}
                icon={<KeyRound size={14} className="mr-2" />}
              />
              {action === "setPassword"
                ? t("secretStorage.settingPassword")
                : t("secretStorage.setRecovery")}
            </Button>
          )}
          {mode === "system" && hasMasterPassword && (
            <Button
              variant="outline"
              className="text-destructive hover:text-destructive"
              onClick={() => void handleRemoveRecovery()}
              disabled={busy}
            >
              <ActionIcon
                busy={action === "removePassword"}
                icon={<Trash2 size={14} className="mr-2" />}
              />
              {t("secretStorage.removeRecovery")}
            </Button>
          )}
        </div>
      </CardContent>
    </Card>
  )
}
