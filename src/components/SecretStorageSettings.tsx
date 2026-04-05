import React, { useEffect, useState } from "react"
import { Shield, Lock, Unlock, AlertTriangle } from "lucide-react"
import { useTranslation } from "react-i18next"
import { useConfig } from "@/contexts/ConfigContext"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"

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
          <div className="border-border bg-secondary/40 rounded-md border p-3 text-sm">
            <div className="flex items-center justify-between gap-3">
              <span>{t("secretStorage.activeBackend")}</span>
              <strong>{backendLabel}</strong>
            </div>
            {secretStatus.message && (
              <p className="text-muted-foreground mt-2 text-xs leading-5">{secretStatus.message}</p>
            )}
          </div>

          {!secretStatus.keyringAvailable && (
            <div className="rounded-md border border-amber-500/40 bg-amber-500/10 p-3 text-sm">
              <div className="flex items-start gap-2">
                <AlertTriangle size={16} className="mt-0.5 shrink-0" />
                <div>
                  <div className="font-medium">{t("secretStorage.keyringUnavailable")}</div>
                  <p className="text-muted-foreground mt-1 text-xs leading-5">
                    {t("secretStorage.keyringUnavailableDesc")}
                  </p>
                </div>
              </div>
            </div>
          )}

          <label className="border-border flex items-center justify-between rounded-md border px-3 py-2 text-sm">
            <div>
              <div className="font-medium">{t("secretStorage.enableVault")}</div>
              <div className="text-muted-foreground mt-1 text-xs">
                {t("secretStorage.enableVaultDesc")}
              </div>
            </div>
            <input
              type="checkbox"
              checked={config.secret_vault_enabled}
              disabled={busy || secretStatus.keyringAvailable}
              onChange={(e) => handleEnableVault(e.target.checked)}
            />
          </label>

          {config.secret_vault_enabled && !secretStatus.keyringAvailable && (
            <div className="border-border space-y-3 rounded-md border p-3">
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
            </div>
          )}

          {error && <div className="text-destructive text-sm">{error}</div>}
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
