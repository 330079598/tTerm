import React from "react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Label } from "@/components/ui/label"
import { cn } from "@/lib/utils"
import type { SshAuthMethod } from "@/types/tab"

const AUTH_METHODS: { value: SshAuthMethod; labelKey: string }[] = [
  { value: "password", labelKey: "ssh.password" },
  { value: "key", labelKey: "ssh.sshKey" },
  { value: "agent", labelKey: "ssh.sshAgent" },
  { value: "interactive", labelKey: "ssh.interactive" },
]

interface AuthMethodPickerProps {
  value: SshAuthMethod
  onChange: (value: SshAuthMethod) => void
}

export const AuthMethodPicker: React.FC<AuthMethodPickerProps> = ({ value, onChange }) => {
  const { t } = useTranslation()

  return (
    <div>
      <Label className="mb-1.5 block">{t("ssh.authMethod")}</Label>
      <div className="flex gap-2" aria-label={t("ssh.authMethod")}>
        {AUTH_METHODS.map((method) => (
          <Button
            key={method.value}
            type="button"
            variant={value === method.value ? "default" : "outline"}
            className={cn(
              "min-w-0 flex-1 px-2",
              value === method.value ? "shadow-none" : "text-muted-foreground"
            )}
            onClick={() => onChange(method.value)}
            aria-pressed={value === method.value}
          >
            <span className="truncate">{t(method.labelKey)}</span>
          </Button>
        ))}
      </div>
    </div>
  )
}
