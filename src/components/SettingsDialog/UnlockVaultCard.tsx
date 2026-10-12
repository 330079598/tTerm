import { Unlock } from "lucide-react"
import { useTranslation } from "react-i18next"

import { ActionIcon, PasswordField } from "@/components/SettingsDialog/SecretFields"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import type { VaultAction } from "@/components/SettingsDialog/types"

interface UnlockVaultCardProps {
  unlockPurpose: "migrate" | "setPassword" | "unlock" | "recover"
  busy: boolean
  action: VaultAction | null
  password: string
  newPassword: string
  confirmPassword: string
  setPassword: (value: string) => void
  setNewPassword: (value: string) => void
  setConfirmPassword: (value: string) => void
  handleUnlock: () => Promise<void>
}

/**
 * Unlocking saved passwords: moving the old vault over, setting the first
 * master password, unlocking with it, or recovering with it.
 */
export function UnlockVaultCard({
  unlockPurpose,
  busy,
  action,
  password,
  newPassword,
  confirmPassword,
  setPassword,
  setNewPassword,
  setConfirmPassword,
  handleUnlock,
}: UnlockVaultCardProps) {
  const { t } = useTranslation()

  return (
    <Card>
      <CardContent className="space-y-3 p-4">
        <div>
          <div className="text-sm font-medium">
            {unlockPurpose === "migrate"
              ? t("secretStorage.migrationTitle")
              : unlockPurpose === "setPassword"
                ? t("secretStorage.setMasterPasswordTitle")
                : t("secretStorage.unlockTitle")}
          </div>
          {unlockPurpose !== "unlock" && (
            <div className="text-muted-foreground mt-1 text-xs leading-5">
              {unlockPurpose === "migrate"
                ? t("secretStorage.migrationDesc")
                : unlockPurpose === "setPassword"
                  ? t("secretStorage.setMasterPasswordDesc")
                  : t("secretStorage.unlockRecoveryDesc")}
            </div>
          )}
        </div>
        <form
          className="space-y-3"
          onSubmit={(event) => {
            event.preventDefault()
            void handleUnlock()
          }}
        >
          {unlockPurpose === "setPassword" ? (
            <>
              <PasswordField
                id="vault-new-password"
                label={t("secretStorage.newPassword")}
                value={newPassword}
                onChange={setNewPassword}
                disabled={busy}
                autoComplete="new-password"
              />
              <PasswordField
                id="vault-confirm-password"
                label={t("secretStorage.confirmNewPassword")}
                value={confirmPassword}
                onChange={setConfirmPassword}
                disabled={busy}
                autoComplete="new-password"
              />
            </>
          ) : (
            <PasswordField
              id="vault-password"
              label={t("secretStorage.masterPassword")}
              value={password}
              onChange={setPassword}
              disabled={busy}
              autoComplete="current-password"
            />
          )}
          <Button
            type="submit"
            disabled={
              busy ||
              (unlockPurpose === "setPassword"
                ? newPassword.length === 0 || confirmPassword.length === 0
                : password.length === 0)
            }
          >
            <ActionIcon busy={action === "unlock"} icon={<Unlock size={14} className="mr-2" />} />
            {action === "unlock"
              ? unlockPurpose === "migrate"
                ? t("secretStorage.migrating")
                : t("secretStorage.unlocking")
              : unlockPurpose === "migrate"
                ? t("secretStorage.migrate")
                : unlockPurpose === "setPassword"
                  ? t("secretStorage.setPassword")
                  : t("secretStorage.unlock")}
          </Button>
        </form>
      </CardContent>
    </Card>
  )
}
