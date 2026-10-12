import { RefreshCw, RotateCcw } from "lucide-react"
import { useTranslation } from "react-i18next"

import { ActionIcon } from "@/components/SettingsDialog/SecretFields"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import type { VaultAction } from "@/components/SettingsDialog/types"
import { useConfig } from "@/contexts/ConfigContext"

interface SystemUnlockFailedCardProps {
  busy: boolean
  action: VaultAction | null
  hasMasterPassword: boolean
  handleRetrySystemUnlock: () => Promise<void>
  handleResetSavedPasswords: () => Promise<void>
  secretStatus: ReturnType<typeof useConfig>["secretStatus"]
}

/**
 * The system credential store did not unlock the saved passwords: try
 * again, or start over without them.
 */
export function SystemUnlockFailedCard({
  busy,
  action,
  hasMasterPassword,
  handleRetrySystemUnlock,
  handleResetSavedPasswords,
  secretStatus,
}: SystemUnlockFailedCardProps) {
  const { t } = useTranslation()

  return (
    <Card>
      <CardContent className="space-y-3 p-4">
        <div>
          <div className="text-sm font-medium">
            {hasMasterPassword
              ? t("secretStorage.resetForgotRecoveryTitle")
              : t("secretStorage.systemUnlockFailedTitle")}
          </div>
          <div className="text-muted-foreground mt-1 text-xs leading-5">
            {t("secretStorage.systemUnlockFailedDesc")}
          </div>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button variant="outline" disabled={busy} onClick={() => void handleRetrySystemUnlock()}>
            <ActionIcon busy={action === "retry"} icon={<RefreshCw size={14} className="mr-2" />} />
            {t("secretStorage.retrySystemUnlock")}
          </Button>
          <Button
            variant="destructive"
            disabled={busy || !secretStatus.keyringAvailable}
            onClick={() => void handleResetSavedPasswords()}
          >
            <ActionIcon busy={action === "reset"} icon={<RotateCcw size={14} className="mr-2" />} />
            {t("secretStorage.reset")}
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}
