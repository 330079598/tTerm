import { KeyRound } from "lucide-react"
import { useTranslation } from "react-i18next"

import { ActionIcon, PasswordField } from "@/components/SettingsDialog/SecretFields"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import type { VaultAction } from "@/components/SettingsDialog/types"

interface ChoosePasswordModeCardProps {
  busy: boolean
  action: VaultAction | null
  newPassword: string
  confirmPassword: string
  setNewPassword: (value: string) => void
  setConfirmPassword: (value: string) => void
  handleConfirmPasswordMode: () => Promise<void>
  clearPasswordFields: () => void
  setChoosingPasswordMode: (choosing: boolean) => void
}

/** Setting the master password that switching to password mode needs. */
export function ChoosePasswordModeCard({
  busy,
  action,
  newPassword,
  confirmPassword,
  setNewPassword,
  setConfirmPassword,
  handleConfirmPasswordMode,
  clearPasswordFields,
  setChoosingPasswordMode,
}: ChoosePasswordModeCardProps) {
  const { t } = useTranslation()

  return (
    <Card>
      <CardContent className="space-y-3 p-4">
        <div>
          <div className="text-sm font-medium">{t("secretStorage.choosePasswordTitle")}</div>
          <div className="text-muted-foreground mt-1 text-xs leading-5">
            {t("secretStorage.choosePasswordDesc")}
          </div>
        </div>
        <PasswordField
          id="choose-master-password"
          label={t("secretStorage.newPassword")}
          value={newPassword}
          onChange={setNewPassword}
          disabled={busy}
          autoComplete="new-password"
          autoFocus
        />
        <PasswordField
          id="choose-master-password-confirm"
          label={t("secretStorage.confirmNewPassword")}
          value={confirmPassword}
          onChange={setConfirmPassword}
          disabled={busy}
          autoComplete="new-password"
        />
        <div className="flex gap-2">
          <Button
            onClick={() => void handleConfirmPasswordMode()}
            disabled={busy || newPassword.length === 0 || confirmPassword.length === 0}
          >
            <ActionIcon busy={action === "mode"} icon={<KeyRound size={14} className="mr-2" />} />
            {t("secretStorage.switchToPassword")}
          </Button>
          <Button
            variant="outline"
            disabled={busy}
            onClick={() => {
              setChoosingPasswordMode(false)
              clearPasswordFields()
            }}
          >
            {t("secretStorage.cancel")}
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}
