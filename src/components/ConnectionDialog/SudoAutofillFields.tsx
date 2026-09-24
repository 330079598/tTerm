import React from "react"
import { useTranslation } from "react-i18next"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"

import { ConnectionForm } from "@/components/ConnectionDialog/types"

interface SudoAutofillFieldsProps {
  form: ConnectionForm
  setForm: React.Dispatch<React.SetStateAction<ConnectionForm>>
  savedSudoPasswordAvailable: boolean
}

export const SudoAutofillFields: React.FC<SudoAutofillFieldsProps> = ({
  form,
  setForm,
  savedSudoPasswordAvailable,
}) => {
  const { t } = useTranslation()
  const hasStoredPassword = savedSudoPasswordAvailable && !form.clearSudoPassword
  const placeholder = hasStoredPassword
    ? t("connection.sudoPasswordSavedPlaceholder")
    : form.authMethod === "password"
      ? t("connection.sudoPasswordLoginFallbackPlaceholder")
      : t("connection.sudoPasswordPlaceholder")

  return (
    <div className="space-y-3 rounded-md border px-3 py-2">
      <div className="flex items-start gap-2">
        <Checkbox
          id="conn-sudo-autofill"
          checked={form.sudoAutofill}
          onCheckedChange={(checked) =>
            setForm((current) => ({ ...current, sudoAutofill: checked }))
          }
        />
        <div className="space-y-0.5">
          <Label htmlFor="conn-sudo-autofill" className="text-sm font-normal">
            {t("connection.sudoAutofill")}
          </Label>
          <p className="text-muted-foreground text-xs">{t("connection.sudoAutofillDesc")}</p>
        </div>
      </div>

      {form.sudoAutofill && (
        <div>
          <div className="mb-1.5 flex items-center gap-2">
            <Label htmlFor="conn-sudo-password">{t("connection.sudoPassword")}</Label>
            {hasStoredPassword && (
              <Badge variant="secondary" className="h-5 px-1.5 text-[10px]">
                {t("connection.savedPasswordBadge")}
              </Badge>
            )}
          </div>
          <div className="flex gap-2">
            <Input
              id="conn-sudo-password"
              type="password"
              autoComplete="new-password"
              value={form.sudoPassword}
              onChange={(e) =>
                setForm((current) => ({
                  ...current,
                  sudoPassword: e.target.value,
                  clearSudoPassword: false,
                }))
              }
              placeholder={placeholder}
              className="flex-1"
            />
            {savedSudoPasswordAvailable && (
              <Button
                type="button"
                variant="outline"
                size="sm"
                disabled={form.clearSudoPassword}
                onClick={() =>
                  setForm((current) => ({ ...current, sudoPassword: "", clearSudoPassword: true }))
                }
              >
                {t("connection.sudoPasswordClear")}
              </Button>
            )}
          </div>
          <p className="text-muted-foreground mt-1 text-xs">
            {form.clearSudoPassword
              ? t("connection.sudoPasswordClearHint")
              : t("connection.sudoPasswordHint")}
          </p>
        </div>
      )}
    </div>
  )
}
