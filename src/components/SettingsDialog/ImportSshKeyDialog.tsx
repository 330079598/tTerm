import React, { useState } from "react"
import { open as openFileDialog } from "@tauri-apps/plugin-dialog"
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
import { PasswordInput } from "@/components/ui/password-input"
import { Spinner } from "@/components/ui/spinner"
import { useToast } from "@/hooks/use-toast"
import { importVaultKey } from "@/hooks/useVaultKeys"
import { toErrorMessage } from "@/lib/utils"

interface ImportSshKeyDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

/** Copies a private key file, and its certificate, into the key vault. */
export const ImportSshKeyDialog: React.FC<ImportSshKeyDialogProps> = ({ open, onOpenChange }) => (
  <Dialog open={open} onOpenChange={onOpenChange}>
    <DialogContent className="sm:max-w-md">
      {/* Mounted while open, so each opening starts empty. */}
      {open && <ImportSshKeyForm onClose={() => onOpenChange(false)} />}
    </DialogContent>
  </Dialog>
)

interface FilePickerFieldProps {
  id: string
  label: string
  value: string
  onChange: (value: string) => void
  hint?: string
}

const FilePickerField: React.FC<FilePickerFieldProps> = ({ id, label, value, onChange, hint }) => {
  const { t } = useTranslation()
  return (
    <div>
      <Label htmlFor={id} className="mb-1.5 block">
        {label}
      </Label>
      <div className="flex gap-2">
        <Input
          id={id}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          className="flex-1"
        />
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={async () => {
            const selected = await openFileDialog({ multiple: false }).catch(() => null)
            if (selected && typeof selected === "string") onChange(selected)
          }}
        >
          {t("ssh.browseKey")}
        </Button>
      </div>
      {hint && <p className="text-muted-foreground mt-1 text-xs">{hint}</p>}
    </div>
  )
}

const ImportSshKeyForm: React.FC<{ onClose: () => void }> = ({ onClose }) => {
  const { t } = useTranslation()
  const { toast } = useToast()
  const [path, setPath] = useState("")
  const [name, setName] = useState("")
  const [passphrase, setPassphrase] = useState("")
  const [rememberPassphrase, setRememberPassphrase] = useState(false)
  const [certificatePath, setCertificatePath] = useState("")
  const [error, setError] = useState<string | null>(null)
  const [importing, setImporting] = useState(false)

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault()
    setImporting(true)
    setError(null)
    try {
      const key = await importVaultKey({
        path: path.trim(),
        name: name.trim() || undefined,
        passphrase: passphrase || undefined,
        rememberPassphrase: rememberPassphrase && passphrase.length > 0,
        certificatePath: certificatePath.trim() || undefined,
      })
      toast({
        title: t("keyVault.importDialog.imported"),
        description: t("keyVault.importDialog.importedDesc", { name: key.name }),
      })
      onClose()
    } catch (caught) {
      setError(toErrorMessage(caught))
    } finally {
      setImporting(false)
    }
  }

  return (
    <form onSubmit={(event) => void handleSubmit(event)} className="space-y-4">
      <DialogHeader>
        <DialogTitle>{t("keyVault.importDialog.title")}</DialogTitle>
        <DialogDescription>{t("keyVault.importDialog.description")}</DialogDescription>
      </DialogHeader>

      <FilePickerField
        id="vault-import-path"
        label={t("keyVault.importDialog.keyFile")}
        value={path}
        onChange={setPath}
      />
      <div>
        <Label htmlFor="vault-import-name" className="mb-1.5 block">
          {t("keyVault.importDialog.name")}
        </Label>
        <Input
          id="vault-import-name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder={t("keyVault.importDialog.namePlaceholder")}
        />
      </div>
      <div>
        <Label htmlFor="vault-import-passphrase" className="mb-1.5 block">
          {t("keyVault.importDialog.passphrase")}
        </Label>
        <PasswordInput
          id="vault-import-passphrase"
          value={passphrase}
          onChange={(e) => setPassphrase(e.target.value)}
        />
        <p className="text-muted-foreground mt-1 text-xs">
          {t("keyVault.importDialog.passphraseHint")}
        </p>
        <div className="mt-2 flex items-center gap-2">
          <Checkbox
            id="vault-import-remember"
            checked={rememberPassphrase && passphrase.length > 0}
            disabled={passphrase.length === 0}
            onCheckedChange={setRememberPassphrase}
          />
          <Label htmlFor="vault-import-remember" className="text-xs font-normal">
            {t("keyVault.importDialog.rememberPassphrase")}
          </Label>
        </div>
      </div>
      <FilePickerField
        id="vault-import-certificate"
        label={t("keyVault.importDialog.certificate")}
        value={certificatePath}
        onChange={setCertificatePath}
        hint={t("keyVault.importDialog.certificateHint")}
      />

      {error && (
        <p className="text-destructive text-xs leading-5" role="alert">
          {error}
        </p>
      )}

      <DialogFooter>
        <Button type="button" variant="outline" onClick={onClose}>
          {t("common.cancel")}
        </Button>
        <Button type="submit" disabled={importing || !path.trim()}>
          {importing && <Spinner size={14} />}
          {t("keyVault.importDialog.submit")}
        </Button>
      </DialogFooter>
    </form>
  )
}
