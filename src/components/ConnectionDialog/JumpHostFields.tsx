import React from "react"
import { open as openFileDialog } from "@tauri-apps/plugin-dialog"
import { ChevronDown, ChevronRight, Server } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Separator } from "@/components/ui/separator"
import { cn } from "@/lib/utils"

import { ConnectionForm } from "@/components/ConnectionDialog/types"

interface JumpHostFieldsProps {
  form: ConnectionForm
  setForm: React.Dispatch<React.SetStateAction<ConnectionForm>>
}

export const JumpHostFields: React.FC<JumpHostFieldsProps> = ({ form, setForm }) => {
  const { t } = useTranslation()

  return (
    <div className="space-y-3">
      <Separator />

      {/* Toggle row */}
      <div className="flex items-center gap-2">
        <Checkbox
          id="use-jump-host"
          checked={form.useJumpHost}
          onCheckedChange={(checked) => setForm((cur) => ({ ...cur, useJumpHost: !!checked }))}
        />
        <Label
          htmlFor="use-jump-host"
          className="flex cursor-pointer items-center gap-1.5 text-sm font-normal"
        >
          <Server size={14} className="text-muted-foreground" />
          {t("jumpHost.enable")}
        </Label>
        {form.useJumpHost ? (
          <ChevronDown size={14} className="text-muted-foreground ml-auto" />
        ) : (
          <ChevronRight size={14} className="text-muted-foreground ml-auto" />
        )}
      </div>

      {form.useJumpHost && (
        <div className="border-muted space-y-3 border-l-2 pl-4">
          {/* Host + Port */}
          <div className="grid grid-cols-3 gap-3">
            <div className="col-span-2">
              <Label htmlFor="jump-host" className="mb-1.5 block">
                {t("jumpHost.host")}
              </Label>
              <Input
                id="jump-host"
                value={form.jumpHost}
                onChange={(e) => setForm((cur) => ({ ...cur, jumpHost: e.target.value }))}
                placeholder="bastion.example.com"
              />
            </div>
            <div>
              <Label htmlFor="jump-port" className="mb-1.5 block">
                {t("connection.port")}
              </Label>
              <Input
                id="jump-port"
                type="number"
                min={1}
                max={65535}
                value={form.jumpPort}
                onChange={(e) => setForm((cur) => ({ ...cur, jumpPort: Number(e.target.value) }))}
              />
            </div>
          </div>

          {/* Username */}
          <div>
            <Label htmlFor="jump-username" className="mb-1.5 block">
              {t("connection.username")}
            </Label>
            <Input
              id="jump-username"
              value={form.jumpUsername}
              onChange={(e) => setForm((cur) => ({ ...cur, jumpUsername: e.target.value }))}
              placeholder="username"
            />
          </div>

          {/* Auth method toggle */}
          <div>
            <Label className="mb-1.5 block">{t("ssh.authMethod")}</Label>
            <div className="flex gap-2">
              <Button
                type="button"
                variant={form.jumpAuthMethod === "password" ? "default" : "outline"}
                className={cn(
                  "flex-1",
                  form.jumpAuthMethod === "password" ? "shadow-none" : "text-muted-foreground"
                )}
                onClick={() => setForm((cur) => ({ ...cur, jumpAuthMethod: "password" }))}
              >
                {t("ssh.password")}
              </Button>
              <Button
                type="button"
                variant={form.jumpAuthMethod === "key" ? "default" : "outline"}
                className={cn(
                  "flex-1",
                  form.jumpAuthMethod === "key" ? "shadow-none" : "text-muted-foreground"
                )}
                onClick={() => setForm((cur) => ({ ...cur, jumpAuthMethod: "key" }))}
              >
                {t("ssh.sshKey")}
              </Button>
            </div>
          </div>

          {/* Password auth */}
          {form.jumpAuthMethod === "password" && (
            <div>
              <Label htmlFor="jump-password" className="mb-1.5 block">
                {t("connection.password")}
              </Label>
              <Input
                id="jump-password"
                type="password"
                value={form.jumpPassword}
                onChange={(e) => setForm((cur) => ({ ...cur, jumpPassword: e.target.value }))}
                placeholder="password"
              />
            </div>
          )}

          {/* Key auth */}
          {form.jumpAuthMethod === "key" && (
            <>
              <div>
                <Label htmlFor="jump-key-path" className="mb-1.5 block">
                  {t("ssh.privateKeyPath")}
                </Label>
                <div className="flex gap-2">
                  <Input
                    id="jump-key-path"
                    value={form.jumpPrivateKeyPath}
                    onChange={(e) =>
                      setForm((cur) => ({ ...cur, jumpPrivateKeyPath: e.target.value }))
                    }
                    placeholder="~/.ssh/id_rsa"
                    className="flex-1"
                  />
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={async () => {
                      const selected = await openFileDialog({ multiple: false }).catch(() => null)
                      if (selected && typeof selected === "string") {
                        setForm((cur) => ({ ...cur, jumpPrivateKeyPath: selected }))
                      }
                    }}
                  >
                    {t("ssh.browseKey")}
                  </Button>
                </div>
              </div>
              <div>
                <Label htmlFor="jump-key-pass" className="mb-1.5 block">
                  {t("ssh.privateKeyPassphrase")}
                </Label>
                <Input
                  id="jump-key-pass"
                  type="password"
                  value={form.jumpPrivateKeyPassphrase}
                  onChange={(e) =>
                    setForm((cur) => ({ ...cur, jumpPrivateKeyPassphrase: e.target.value }))
                  }
                  placeholder="passphrase"
                />
              </div>
            </>
          )}
        </div>
      )}
    </div>
  )
}
