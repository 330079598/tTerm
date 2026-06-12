import React, { useState } from "react"
import { Lock, Unlock } from "lucide-react"
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

interface VaultStartupUnlockDialogProps {
  open: boolean
  onClose: () => void
}

export const VaultStartupUnlockDialog: React.FC<VaultStartupUnlockDialogProps> = ({
  open,
  onClose,
}) => {
  const { t } = useTranslation()
  const { unlockSecretVault } = useConfig()
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
      await unlockSecretVault(password, true)
      setPassword("")
      onClose()
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error))
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
            <DialogDescription>{t("secretStorage.startupUnlockDesc")}</DialogDescription>
          </DialogHeader>

          <div className="space-y-2">
            <Label htmlFor="startup-vault-password">{t("secretStorage.vaultPassword")}</Label>
            <Input
              id="startup-vault-password"
              autoFocus
              type="password"
              value={password}
              placeholder={t("secretStorage.vaultPasswordPlaceholder")}
              disabled={busy}
              onChange={(event) => setPassword(event.target.value)}
            />
            {error && <p className="text-destructive text-sm">{error}</p>}
          </div>

          <DialogFooter>
            <Button type="button" variant="outline" disabled={busy} onClick={onClose}>
              {t("secretStorage.skipStartupUnlock")}
            </Button>
            <Button type="submit" disabled={busy || password.length === 0}>
              <Unlock size={14} className="mr-2" />
              {t("secretStorage.unlockVault")}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
