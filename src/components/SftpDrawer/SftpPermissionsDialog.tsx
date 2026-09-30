import React, { useState } from "react"
import { Loader2 } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
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

import { formatOctalMode, parseOctalMode, PERMISSION_BITS } from "@/components/SftpDrawer/sftpView"
import type { SftpDirectoryEntry } from "@/components/SftpDrawer/types"

const ACCESS_KINDS = ["read", "write", "execute"] as const
/** setuid, setgid and sticky: kept as they are when only the checkboxes change. */
const SPECIAL_BITS = 0o7000
const DEFAULT_MODE = 0o644

interface SftpPermissionsDialogProps {
  /** The entries to change; the dialog is open while there are any. */
  entries: SftpDirectoryEntry[]
  onClose: () => void
  /** Applies `mode` to every entry; rejects with the reason on failure. */
  onApply: (entries: SftpDirectoryEntry[], mode: number) => Promise<void>
}

export const SftpPermissionsDialog: React.FC<SftpPermissionsDialogProps> = ({
  entries,
  onClose,
  onApply,
}) => {
  if (entries.length === 0) return null

  // Remounting per target resets the form without an effect.
  return (
    <SftpPermissionsForm
      key={entries.map((entry) => entry.path).join("\n")}
      entries={entries}
      onClose={onClose}
      onApply={onApply}
    />
  )
}

const SftpPermissionsForm: React.FC<SftpPermissionsDialogProps> = ({
  entries,
  onClose,
  onApply,
}) => {
  const { t } = useTranslation()
  const [octal, setOctal] = useState(() => formatOctalMode(entries[0].mode ?? DEFAULT_MODE))
  const [isApplying, setIsApplying] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const mode = parseOctalMode(octal)

  const toggleBit = (bit: number, checked: boolean) => {
    const current = mode ?? 0
    setOctal(formatOctalMode(checked ? current | bit : current & ~bit))
  }

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault()
    if (mode === null || isApplying) return
    setIsApplying(true)
    setError(null)
    try {
      await onApply(entries, mode)
      onClose()
    } catch (applyError) {
      setError(String(applyError))
      setIsApplying(false)
    }
  }

  return (
    <Dialog open onOpenChange={(open) => !open && !isApplying && onClose()}>
      <DialogContent className="sm:max-w-md">
        <form onSubmit={handleSubmit} className="space-y-4">
          <DialogHeader>
            <DialogTitle>{t("sftp.permissions.title")}</DialogTitle>
            <DialogDescription className="break-all">
              {entries.length === 1
                ? entries[0].path
                : t("sftp.permissions.multiple", { count: entries.length })}
            </DialogDescription>
          </DialogHeader>

          <div
            className="grid grid-cols-[minmax(0,1fr)_repeat(3,4.5rem)] items-center gap-x-2 gap-y-2.5 text-sm"
            role="group"
            aria-label={t("sftp.permissions.title")}
          >
            <span />
            {ACCESS_KINDS.map((access) => (
              <span key={access} className="text-muted-foreground text-center text-xs">
                {t(`sftp.permissions.${access}`)}
              </span>
            ))}
            {PERMISSION_BITS.map((bits) => (
              <React.Fragment key={bits.scope}>
                <span>{t(`sftp.permissions.${bits.scope}`)}</span>
                {ACCESS_KINDS.map((access) => (
                  <span key={access} className="flex justify-center">
                    <Checkbox
                      checked={mode !== null && (mode & bits[access]) !== 0}
                      disabled={isApplying}
                      onCheckedChange={(checked) => toggleBit(bits[access], checked)}
                      aria-label={`${t(`sftp.permissions.${bits.scope}`)} ${t(`sftp.permissions.${access}`)}`}
                    />
                  </span>
                ))}
              </React.Fragment>
            ))}
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="sftp-permissions-octal">{t("sftp.permissions.octal")}</Label>
            <Input
              id="sftp-permissions-octal"
              data-dialog-initial-focus="true"
              value={octal}
              maxLength={4}
              inputMode="numeric"
              className="w-28 font-mono"
              disabled={isApplying}
              aria-invalid={mode === null}
              aria-describedby="sftp-permissions-hint"
              onChange={(event) => setOctal(event.target.value.replace(/[^0-7]/g, ""))}
            />
            <p id="sftp-permissions-hint" className="text-muted-foreground text-xs leading-5">
              {mode !== null && (mode & SPECIAL_BITS) !== 0
                ? t("sftp.permissions.specialHint")
                : t("sftp.permissions.octalHint")}
            </p>
          </div>

          {error && (
            <p role="alert" className="text-destructive text-xs leading-5 break-all">
              {error}
            </p>
          )}

          <DialogFooter>
            <Button type="button" variant="outline" disabled={isApplying} onClick={onClose}>
              {t("common.cancel")}
            </Button>
            <Button type="submit" disabled={mode === null || isApplying}>
              {isApplying && <Loader2 className="animate-spin" />}
              {t("sftp.permissions.apply")}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
