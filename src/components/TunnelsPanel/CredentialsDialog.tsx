import React, { useState } from "react"
import { KeyRound, Loader2 } from "lucide-react"
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
import type { CredentialRequest } from "@/types/tunnel"
import { credentialFieldKey } from "@/components/TunnelsPanel/tunnelUtils"

interface CredentialsDialogProps {
  tunnelName: string
  requests: CredentialRequest[]
  busy: boolean
  onSubmit: (values: Record<string, string>, remember: boolean) => void
  onCancel: () => void
}

/** Asks for the passwords and key passphrases a tunnel needs before it can connect. */
export const CredentialsDialog: React.FC<CredentialsDialogProps> = ({
  tunnelName,
  requests,
  busy,
  onSubmit,
  onCancel,
}) => {
  const { t } = useTranslation()
  const [values, setValues] = useState<Record<string, string>>({})
  const [remember, setRemember] = useState(false)
  const hasPassword = requests.some((request) => request.kind === "password")
  const complete = requests.every((request) => (values[credentialFieldKey(request)] ?? "") !== "")

  const submit = () => {
    if (complete && !busy) onSubmit(values, remember)
  }

  return (
    <Dialog open onOpenChange={(open) => !open && !busy && onCancel()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <div className="flex items-center gap-3">
            <div className="bg-muted text-muted-foreground flex size-9 shrink-0 items-center justify-center rounded-lg border">
              <KeyRound size={17} />
            </div>
            <div className="min-w-0">
              <DialogTitle>
                {t("tunnels.credentials.title", { defaultValue: "Credentials required" })}
              </DialogTitle>
              <DialogDescription className="mt-1">
                {t("tunnels.credentials.description", {
                  name: tunnelName,
                  defaultValue: "“{{name}}” needs the following to connect.",
                })}
              </DialogDescription>
            </div>
          </div>
        </DialogHeader>

        <form
          className="space-y-4"
          onSubmit={(event) => {
            event.preventDefault()
            submit()
          }}
        >
          {requests.map((request, index) => {
            const key = credentialFieldKey(request)
            const id = `tunnel-credential-${key}`
            return (
              <div key={key}>
                <Label htmlFor={id}>
                  {request.kind === "password"
                    ? t("tunnels.credentials.password", {
                        label: request.label,
                        defaultValue: "Password for {{label}}",
                      })
                    : t("tunnels.credentials.passphrase", {
                        label: request.label,
                        defaultValue: "Key passphrase for {{label}}",
                      })}
                </Label>
                <Input
                  id={id}
                  className="mt-1.5"
                  type="password"
                  autoFocus={index === 0}
                  autoComplete="off"
                  disabled={busy}
                  onKeyDown={(event) => {
                    // Do not rely on implicit form submission; WebKit skips hidden submit buttons.
                    if (event.key === "Enter") {
                      event.preventDefault()
                      submit()
                    }
                  }}
                  value={values[key] ?? ""}
                  onChange={(event) =>
                    setValues((current) => ({ ...current, [key]: event.target.value }))
                  }
                />
                {request.incorrect && (
                  <p className="text-destructive mt-1 text-xs" role="alert">
                    {t("tunnels.credentials.incorrect", {
                      defaultValue: "That passphrase did not unlock the key. Try again.",
                    })}
                  </p>
                )}
              </div>
            )
          })}

          {hasPassword && (
            <label className="flex cursor-pointer items-start gap-2 text-sm">
              <Checkbox
                checked={remember}
                disabled={busy}
                onCheckedChange={setRemember}
                className="mt-0.5"
              />
              <span>
                {t("tunnels.credentials.remember", {
                  defaultValue: "Save in secure storage so I am not asked again",
                })}
              </span>
            </label>
          )}
        </form>

        <DialogFooter>
          <Button type="button" variant="outline" disabled={busy} onClick={onCancel}>
            {t("common.cancel", { defaultValue: "Cancel" })}
          </Button>
          <Button type="button" disabled={!complete || busy} onClick={submit}>
            {busy && <Loader2 className="animate-spin" />}
            {t("tunnels.credentials.connect", { defaultValue: "Connect" })}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
