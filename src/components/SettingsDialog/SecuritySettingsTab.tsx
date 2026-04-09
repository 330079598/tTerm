import React from "react"
import { AlertTriangle, Lock, Unlock } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Switch } from "@/components/ui/switch"

import { SecretStatusState } from "@/components/SettingsDialog/types"

interface SecuritySettingsTabProps {
  backendLabel: string
  configSecretVaultEnabled: boolean
  handleEnableVault: (checked: boolean) => Promise<void>
  handleLock: () => Promise<void>
  handleUnlock: () => Promise<void>
  password: string
  secretBusy: boolean
  secretError: string | null
  secretStatus: SecretStatusState
  setPassword: React.Dispatch<React.SetStateAction<string>>
}

export const SecuritySettingsTab: React.FC<SecuritySettingsTabProps> = ({
  backendLabel,
  configSecretVaultEnabled,
  handleEnableVault,
  handleLock,
  handleUnlock,
  password,
  secretBusy,
  secretError,
  secretStatus,
  setPassword,
}) => {
  const { t } = useTranslation()

  return (
    <ScrollArea className="h-full pr-4">
      <div className="space-y-4">
        <p className="text-muted-foreground text-sm">{t("secretStorage.description")}</p>

        <Card>
          <CardContent className="space-y-2 p-4 text-sm">
            <div className="flex items-center justify-between gap-3">
              <span>{t("secretStorage.activeBackend")}</span>
              <Badge variant="secondary">{backendLabel}</Badge>
            </div>
            {secretStatus.message && (
              <p className="text-muted-foreground text-xs leading-5">{secretStatus.message}</p>
            )}
          </CardContent>
        </Card>

        {!secretStatus.keyringAvailable && (
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
        )}

        <Card>
          <CardContent className="flex items-center justify-between gap-4 p-4">
            <div>
              <div className="text-sm font-medium">{t("secretStorage.enableVault")}</div>
              <div className="text-muted-foreground mt-1 text-xs">
                {t("secretStorage.enableVaultDesc")}
              </div>
            </div>
            <Switch
              checked={configSecretVaultEnabled}
              disabled={secretBusy || secretStatus.keyringAvailable}
              onCheckedChange={handleEnableVault}
            />
          </CardContent>
        </Card>

        {configSecretVaultEnabled && !secretStatus.keyringAvailable && (
          <Card>
            <CardContent className="space-y-3 p-4">
              <div>
                <Label htmlFor="vault-password" className="mb-1.5 block">
                  {t("secretStorage.vaultPassword")}
                </Label>
                <Input
                  id="vault-password"
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  placeholder={t("secretStorage.vaultPasswordPlaceholder")}
                  disabled={secretBusy}
                />
              </div>
              <div className="flex gap-2">
                {!secretStatus.strongholdUnlocked ? (
                  <Button onClick={handleUnlock} disabled={secretBusy || password.length === 0}>
                    <Unlock size={14} className="mr-2" />
                    {t("secretStorage.unlockVault")}
                  </Button>
                ) : (
                  <Button variant="outline" onClick={handleLock} disabled={secretBusy}>
                    <Lock size={14} className="mr-2" />
                    {t("secretStorage.lockVault")}
                  </Button>
                )}
              </div>
            </CardContent>
          </Card>
        )}

        {secretError && (
          <Alert className="border-destructive/40 bg-destructive/10 text-destructive">
            <AlertTitle>{t("secretStorage.title")}</AlertTitle>
            <AlertDescription className="mt-1 text-sm text-current">{secretError}</AlertDescription>
          </Alert>
        )}
      </div>
    </ScrollArea>
  )
}
