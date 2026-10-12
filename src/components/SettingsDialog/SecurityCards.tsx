import React from "react"
import { AlertTriangle, Lock } from "lucide-react"
import { useTranslation } from "react-i18next"

import { ActionIcon } from "@/components/SettingsDialog/SecretFields"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { Switch } from "@/components/ui/switch"

/** Shown when the OS credential store can't be reached. */
export const KeyringUnavailableAlert: React.FC = () => {
  const { t } = useTranslation()
  return (
    <Alert className="border-amber-500/40 bg-amber-500/10">
      <div className="flex items-start gap-2">
        <AlertTriangle size={16} className="mt-0.5 shrink-0" />
        <div>
          <AlertTitle>{t("secretStorage.keyringUnavailable")}</AlertTitle>
          <AlertDescription className="mt-1 text-xs leading-5">
            {t("secretStorage.keyringUnavailableDesc")}
          </AlertDescription>
        </div>
      </div>
    </Alert>
  )
}

interface LockVaultCardProps {
  busy: boolean
  locking: boolean
  onLock: () => void
}

/** Locks saved passwords again in master password mode. */
export const LockVaultCard: React.FC<LockVaultCardProps> = ({ busy, locking, onLock }) => {
  const { t } = useTranslation()
  return (
    <Card>
      <CardContent className="flex items-center justify-between gap-4 p-4">
        <div>
          <div className="text-sm font-medium">{t("secretStorage.lock")}</div>
          <div className="text-muted-foreground mt-1 text-xs">{t("secretStorage.lockedDesc")}</div>
        </div>
        <Button variant="outline" onClick={onLock} disabled={busy}>
          <ActionIcon busy={locking} icon={<Lock size={14} className="mr-2" />} />
          {t("secretStorage.lock")}
        </Button>
      </CardContent>
    </Card>
  )
}

interface StartupUnlockPromptCardProps {
  checked: boolean
  busy: boolean
  onCheckedChange: (checked: boolean) => void
}

/** Whether the app asks for the master password when it starts. */
export const StartupUnlockPromptCard: React.FC<StartupUnlockPromptCardProps> = ({
  checked,
  busy,
  onCheckedChange,
}) => {
  const { t } = useTranslation()
  return (
    <Card>
      <CardContent className="flex items-center justify-between gap-4 p-4">
        <div>
          <div className="text-sm font-medium">{t("secretStorage.promptUnlockOnStartup")}</div>
          <div className="text-muted-foreground mt-1 text-xs leading-5">
            {t("secretStorage.promptUnlockOnStartupDesc")}
          </div>
        </div>
        <Switch checked={checked} disabled={busy} onCheckedChange={onCheckedChange} />
      </CardContent>
    </Card>
  )
}

/** The error of the last secret store action. */
export const VaultErrorAlert: React.FC<{ error: string }> = ({ error }) => {
  const { t } = useTranslation()
  return (
    <Alert className="border-destructive/40 bg-destructive/10 text-destructive">
      <AlertTitle>{t("secretStorage.title")}</AlertTitle>
      <AlertDescription className="mt-1 text-sm text-current" role="alert">
        {error}
      </AlertDescription>
    </Alert>
  )
}
