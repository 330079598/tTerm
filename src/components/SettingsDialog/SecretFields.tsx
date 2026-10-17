import React from "react"
import { useTranslation } from "react-i18next"

import { Label } from "@/components/ui/label"
import { PasswordInput } from "@/components/ui/password-input"
import { Spinner } from "@/components/ui/spinner"

export interface PasswordFieldProps {
  id: string
  label: string
  value: string
  onChange: (value: string) => void
  disabled: boolean
  autoComplete: "current-password" | "new-password"
  autoFocus?: boolean
}

export const PasswordField: React.FC<PasswordFieldProps> = ({
  id,
  label,
  value,
  onChange,
  disabled,
  autoComplete,
  autoFocus,
}) => {
  const { t } = useTranslation()
  return (
    <div>
      <Label htmlFor={id} className="mb-1.5 block">
        {label}
      </Label>
      <PasswordInput
        id={id}
        autoComplete={autoComplete}
        autoFocus={autoFocus}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={t("secretStorage.masterPasswordPlaceholder")}
        disabled={disabled}
      />
    </div>
  )
}

export const ActionIcon: React.FC<{ busy: boolean; icon: React.ReactNode }> = ({ busy, icon }) =>
  busy ? <Spinner size={14} className="mr-2" /> : <>{icon}</>
