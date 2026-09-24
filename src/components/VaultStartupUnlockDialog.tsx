import React, { useState } from "react"
import { Loader2, Lock, Unlock } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { useConfig } from "@/contexts/ConfigContext"
import { useToast } from "@/hooks/use-toast"
import { toErrorMessage } from "@/lib/utils"

interface VaultStartupUnlockDialogProps {
  open: boolean
  onClose: () => void
}

export const VaultStartupUnlockDialog: React.FC<VaultStartupUnlockDialogProps> = ({
  open,
  onClose,
}) => {
  const { t } = useTranslation()
  const { secretStatus, unlockSecretVault } = useConfig()
  const { toast } = useToast()
  const [password, setPassword] = useState("")
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const handleUnlock = async (event: React.FormEvent) => {
    event.preventDefault()
    if (!password) {
      return
    }

    setBusy(true)
    setError(null)
    try {
      const wasPending = secretStatus.migrationPending
      await unlockSecretVault(password)
      setPassword("")
      onClose()
      toast({
        title: wasPending ? t("secretStorage.migrated") : t("secretStorage.unlocked"),
        description: wasPending ? t("secretStorage.migratedDesc") : t("secretStorage.unlockedDesc"),
        variant: "success",
      })
    } catch (error) {
      setError(toErrorMessage(error))
    } finally {
      setBusy(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={(nextOpen) => !nextOpen && onClose()}>
      <DialogContent className="sm:max-w-md">
        <form className="space-y-4" onSubmit={handleUnlock}>
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <Lock className="size-4" />
              {t("secretStorage.startupUnlockTitle")}
            </DialogTitle>
            <DialogDescription>
              {secretStatus.migrationPending
                ? t("secretStorage.migrationDesc")
                : secretStatus.storageMode === "system"
                  ? t("secretStorage.startupRecoveryDesc")
                  : t("secretStorage.startupUnlockDesc")}
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-2">
            <Label htmlFor="startup-vault-password">{t("secretStorage.masterPassword")}</Label>
            <Input
              id="startup-vault-password"
              autoFocus
              type="password"
              value={password}
              placeholder={t("secretStorage.masterPasswordPlaceholder")}
              disabled={busy}
              onChange={(event) => setPassword(event.target.value)}
            />
            {error && (
              <p className="text-destructive mt-1 text-xs" role="alert">
                {error}
              </p>
            )}
          </div>

          <DialogFooter>
            <Button type="button" variant="outline" disabled={busy} onClick={onClose}>
              {t("secretStorage.skipStartupUnlock")}
            </Button>
            <Button type="submit" disabled={busy || password.length === 0}>
              {busy ? (
                <Loader2 size={14} className="mr-2 animate-spin" />
              ) : (
                <Unlock size={14} className="mr-2" />
              )}
              {busy
                ? secretStatus.migrationPending
                  ? t("secretStorage.migrating")
                  : t("secretStorage.unlocking")
                : secretStatus.migrationPending
                  ? t("secretStorage.migrate")
                  : t("secretStorage.unlock")}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
