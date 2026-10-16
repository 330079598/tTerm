import React, { useState } from "react"
import { open as openFileDialog } from "@tauri-apps/plugin-dialog"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import { useToast } from "@/hooks/use-toast"
import { importVaultKey, useVaultKeys } from "@/hooks/useVaultKeys"
import { isVaultKeyPath, shortFingerprint, vaultKeyId, vaultKeyReference } from "@/lib/keyVault"
import { cn, toErrorMessage } from "@/lib/utils"

type KeySource = "file" | "vault"

interface PrivateKeyFieldProps {
  id: string
  /** A key file path, or a `tterm-key:` reference to a key in the vault. */
  value: string
  onChange: (value: string) => void
  /** The passphrase typed for the key, checked when a key file goes into the vault. */
  passphrase: string
  error?: string
}

/**
 * The private key a connection signs in with: a key file on this device, or
 * a key from the key vault, which sync and backups take to other devices.
 */
export const PrivateKeyField: React.FC<PrivateKeyFieldProps> = ({
  id,
  value,
  onChange,
  passphrase,
  error,
}) => {
  const { t } = useTranslation()
  // A vault key shows as one whenever the value names one, even when the
  // profile is filled in after the field appeared; an empty value shows as
  // the source the user picked last.
  const [vaultChosen, setVaultChosen] = useState(false)
  const source: KeySource = isVaultKeyPath(value) || vaultChosen ? "vault" : "file"
  const errorId = `${id}-error`

  const chooseSource = (next: KeySource) => {
    if (next === source) return
    setVaultChosen(next === "vault")
    // A path means nothing to the other source.
    onChange("")
  }

  return (
    <div>
      <div className="mb-1.5 flex items-center justify-between gap-2">
        <Label htmlFor={id}>
          {source === "vault" ? t("keyVault.field.vaultKey") : t("ssh.privateKeyPath")}
        </Label>
        <div className="flex gap-1" role="group" aria-label={t("keyVault.field.source")}>
          {(["file", "vault"] as const).map((option) => (
            <Button
              key={option}
              type="button"
              size="sm"
              variant={source === option ? "secondary" : "ghost"}
              className={cn("h-6 px-2 text-xs", source !== option && "text-muted-foreground")}
              aria-pressed={source === option}
              onClick={() => chooseSource(option)}
            >
              {t(`keyVault.field.${option}`)}
            </Button>
          ))}
        </div>
      </div>
      {source === "file" ? (
        <KeyFileInput
          id={id}
          value={value}
          onChange={onChange}
          passphrase={passphrase}
          errorId={error ? errorId : undefined}
          onSavedToVault={onChange}
        />
      ) : (
        <VaultKeySelect id={id} value={value} onChange={onChange} />
      )}
      {error && (
        <p id={errorId} className="text-destructive mt-1 text-xs" role="alert">
          {error}
        </p>
      )}
    </div>
  )
}

interface KeyFileInputProps {
  id: string
  value: string
  onChange: (value: string) => void
  passphrase: string
  errorId?: string
  /** Called with the reference to the key once the file is in the vault. */
  onSavedToVault: (reference: string) => void
}

const KeyFileInput: React.FC<KeyFileInputProps> = ({
  id,
  value,
  onChange,
  passphrase,
  errorId,
  onSavedToVault,
}) => {
  const { t } = useTranslation()
  const { toast } = useToast()
  const [saving, setSaving] = useState(false)

  const saveToVault = async () => {
    setSaving(true)
    try {
      const key = await importVaultKey({ path: value.trim(), passphrase: passphrase || undefined })
      onSavedToVault(vaultKeyReference(key.id))
      toast({
        title: t("keyVault.field.savedToVault"),
        description: t("keyVault.field.savedToVaultDesc", { name: key.name }),
      })
    } catch (caught) {
      toast({
        title: t("keyVault.field.saveToVaultFailed"),
        description: toErrorMessage(caught),
        variant: "destructive",
      })
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="flex gap-2">
      <Input
        id={id}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder="~/.ssh/id_rsa"
        className="flex-1"
        aria-invalid={!!errorId}
        aria-describedby={errorId}
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
      <Button
        type="button"
        variant="outline"
        size="sm"
        disabled={saving || !value.trim()}
        title={t("keyVault.field.saveToVaultHint")}
        onClick={() => void saveToVault()}
      >
        {t("keyVault.field.saveToVault")}
      </Button>
    </div>
  )
}

interface VaultKeySelectProps {
  id: string
  value: string
  onChange: (value: string) => void
}

const VaultKeySelect: React.FC<VaultKeySelectProps> = ({ id, value, onChange }) => {
  const { t } = useTranslation()
  const { keys, loaded, loadError } = useVaultKeys()
  const selectedId = vaultKeyId(value)
  const selected = keys.find((key) => key.id === selectedId)
  const missing = loaded && !loadError && selectedId !== null && !selected

  const hint = loadError
    ? t("keyVault.field.unreadable", { error: loadError })
    : !loaded
      ? null
      : missing
        ? t("keyVault.field.missing")
        : keys.length === 0
          ? t("keyVault.field.noKeys")
          : selected?.passphraseSaved
            ? t("keyVault.field.passphraseSaved")
            : null

  return (
    <>
      <Select
        id={id}
        value={selectedId ?? ""}
        onChange={(e) => onChange(e.target.value ? vaultKeyReference(e.target.value) : "")}
      >
        <option value="" disabled>
          {t("keyVault.field.chooseKey")}
        </option>
        {keys.map((key) => (
          <option key={key.id} value={key.id}>
            {`${key.name} · ${key.algorithm} · ${shortFingerprint(key.fingerprint)}`}
          </option>
        ))}
        {selectedId && !selected && (
          <option value={selectedId}>{t("keyVault.field.unknownKey")}</option>
        )}
      </Select>
      {hint && (
        <p
          className={cn(
            "mt-1 text-xs",
            missing || loadError ? "text-destructive" : "text-muted-foreground"
          )}
        >
          {hint}
        </p>
      )}
    </>
  )
}
