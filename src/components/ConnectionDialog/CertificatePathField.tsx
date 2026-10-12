import React from "react"
import { open as openFileDialog } from "@tauri-apps/plugin-dialog"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"

interface CertificatePathFieldProps {
  id: string
  value: string
  onChange: (value: string) => void
}

/**
 * The OpenSSH certificate a private key signs in with, for hosts that trust a
 * certificate authority. Left empty, the certificate stored beside the key as
 * `<key>-cert.pub` is used when there is one, as `ssh` does.
 */
export const CertificatePathField: React.FC<CertificatePathFieldProps> = ({
  id,
  value,
  onChange,
}) => {
  const { t } = useTranslation()

  return (
    <div>
      <Label htmlFor={id} className="mb-1.5 block">
        {t("ssh.certificatePath")}
      </Label>
      <div className="flex gap-2">
        <Input
          id={id}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder="~/.ssh/id_ed25519-cert.pub"
          className="flex-1"
          aria-describedby={`${id}-hint`}
        />
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={async () => {
            const selected = await openFileDialog({ multiple: false }).catch(() => null)
            if (selected && typeof selected === "string") {
              onChange(selected)
            }
          }}
        >
          {t("ssh.browseKey")}
        </Button>
      </div>
      <p id={`${id}-hint`} className="text-muted-foreground mt-1 text-xs">
        {t("ssh.certificatePathHint")}
      </p>
    </div>
  )
}
