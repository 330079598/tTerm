import React, { useEffect, useState } from "react"
import { Shield, Lock, Unlock, AlertTriangle } from "lucide-react"
import { useTranslation } from "react-i18next"
import { useConfig } from "@/contexts/ConfigContext"
import {
  DialogDescription,
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Switch } from "@/components/ui/switch"

interface SecretStorageSettingsProps {
  onClose: () => void
}

export const SecretStorageSettings: React.FC<SecretStorageSettingsProps> = ({ onClose }) => {
  const { t } = useTranslation()
  const {
    config,
    secretStatus,
    refreshSecretStatus,
    setSecretVaultEnabled,
    unlockSecretVault,
    lockSecretVault,
  } = useConfig()
  const [password, setPassword] = useState("")
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    refreshSecretStatus().catch(() => {})
  }, [refreshSecretStatus])

  const handleEnableVault = async (checked: boolean) => {
    setBusy(true)
    setError(null)
    try {
      await setSecretVaultEnabled(checked)
      if (!checked) {
        setPassword("")
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const handleUnlock = async () => {
    setBusy(true)
    setError(null)
    try {
      await unlockSecretVault(password, config.secret_vault_enabled)
      setPassword("")
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const handleLock = async () => {
    setBusy(true)
    setError(null)
    try {
      await lockSecretVault()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const backendLabel =
    secretStatus.activeBackend === "system"
      ? t("secretStorage.backends.system")
      : secretStatus.activeBackend === "vault"
        ? t("secretStorage.backends.vault")
        : t("secretStorage.backends.memory")

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Shield size={16} />
            {t("secretStorage.title")}
          </DialogTitle>
          <DialogDescription>{t("secretStorage.description")}</DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-2">
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
                checked={config.secret_vault_enabled}
                disabled={busy || secretStatus.keyringAvailable}
                onCheckedChange={handleEnableVault}
              />
            </CardContent>
          </Card>

          {config.secret_vault_enabled && !secretStatus.keyringAvailable && (
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
                    disabled={busy}
                  />
                </div>
                <div className="flex gap-2">
                  {!secretStatus.strongholdUnlocked ? (
                    <Button onClick={handleUnlock} disabled={busy || password.length === 0}>
                      <Unlock size={14} className="mr-2" />
                      {t("secretStorage.unlockVault")}
                    </Button>
                  ) : (
                    <Button variant="outline" onClick={handleLock} disabled={busy}>
                      <Lock size={14} className="mr-2" />
                      {t("secretStorage.lockVault")}
                    </Button>
                  )}
                </div>
              </CardContent>
            </Card>
          )}

          {error && (
            <Alert className="border-destructive/40 bg-destructive/10 text-destructive">
              <AlertTitle>{t("secretStorage.title")}</AlertTitle>
              <AlertDescription className="mt-1 text-sm text-current">{error}</AlertDescription>
            </Alert>
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            {t("secretStorage.close")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
