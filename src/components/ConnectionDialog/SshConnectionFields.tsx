import React from "react"
import { open as openFileDialog } from "@tauri-apps/plugin-dialog"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { cn } from "@/lib/utils"

import { ConnectionForm } from "@/components/ConnectionDialog/types"

interface SshConnectionFieldsProps {
  form: ConnectionForm
  setForm: React.Dispatch<React.SetStateAction<ConnectionForm>>
  matchingGroups: string[]
  nameError: string | null
  setNameError: React.Dispatch<React.SetStateAction<string | null>>
  showGroupDropdown: boolean
  setShowGroupDropdown: React.Dispatch<React.SetStateAction<boolean>>
  titlePlaceholder: string
}

export const SshConnectionFields: React.FC<SshConnectionFieldsProps> = ({
  form,
  setForm,
  matchingGroups,
  nameError,
  setNameError,
  showGroupDropdown,
  setShowGroupDropdown,
  titlePlaceholder,
}) => {
  const { t } = useTranslation()

  return (
    <>
      <div>
        <Label htmlFor="conn-title" className="mb-1.5 block">
          {t("connection.title")}
        </Label>
        <Input
          id="conn-title"
          value={form.title}
          onChange={(e) => {
            setForm((current) => ({ ...current, title: e.target.value }))
            setNameError(null)
          }}
          placeholder={titlePlaceholder}
        />
        {nameError && <p className="text-destructive mt-1 text-xs">{nameError}</p>}
      </div>

      <div className="relative">
        <Label htmlFor="conn-group" className="mb-1.5 block">
          {t("connection.group")}
        </Label>
        <Input
          id="conn-group"
          value={form.group}
          onChange={(e) => {
            setForm((current) => ({ ...current, group: e.target.value }))
            setShowGroupDropdown(true)
          }}
          onFocus={() => setShowGroupDropdown(true)}
          onBlur={() => setTimeout(() => setShowGroupDropdown(false), 150)}
          placeholder={t("connection.groupPlaceholder")}
          autoComplete="off"
        />
        {showGroupDropdown && matchingGroups.length > 0 && (
          <Card className="absolute top-full right-0 left-0 z-20 mt-1 max-h-40 overflow-y-auto rounded-md">
            <CardContent className="p-1">
              {matchingGroups.map((group) => (
                <button
                  key={group}
                  type="button"
                  className="hover:bg-muted w-full rounded-sm px-3 py-1.5 text-left text-sm"
                  onMouseDown={() => {
                    setForm((current) => ({ ...current, group }))
                    setShowGroupDropdown(false)
                  }}
                >
                  {group}
                </button>
              ))}
            </CardContent>
          </Card>
        )}
      </div>

      <div className="grid grid-cols-3 gap-3">
        <div className="col-span-2">
          <Label htmlFor="conn-host" className="mb-1.5 block">
            {t("connection.host")}
          </Label>
          <Input
            id="conn-host"
            value={form.host}
            onChange={(e) => setForm((current) => ({ ...current, host: e.target.value }))}
            placeholder="hostname or IP"
            required
          />
        </div>
        <div>
          <Label htmlFor="conn-port" className="mb-1.5 block">
            {t("connection.port")}
          </Label>
          <Input
            id="conn-port"
            type="number"
            value={form.port}
            onChange={(e) => setForm((current) => ({ ...current, port: Number(e.target.value) }))}
          />
        </div>
      </div>

      <div>
        <Label htmlFor="conn-user" className="mb-1.5 block">
          {t("connection.username")}
        </Label>
        <Input
          id="conn-user"
          value={form.username}
          onChange={(e) => setForm((current) => ({ ...current, username: e.target.value }))}
          placeholder="username"
        />
      </div>

      <div>
        <Label className="mb-1.5 block">{t("ssh.authMethod")}</Label>
        <div className="flex gap-2">
          <Button
            type="button"
            variant={form.authMethod === "password" ? "default" : "outline"}
            className={cn(
              "flex-1",
              form.authMethod === "password" ? "shadow-none" : "text-muted-foreground"
            )}
            onClick={() => setForm((current) => ({ ...current, authMethod: "password" }))}
          >
            {t("ssh.password")}
          </Button>
          <Button
            type="button"
            variant={form.authMethod === "key" ? "default" : "outline"}
            className={cn(
              "flex-1",
              form.authMethod === "key" ? "shadow-none" : "text-muted-foreground"
            )}
            onClick={() => setForm((current) => ({ ...current, authMethod: "key" }))}
          >
            {t("ssh.sshKey")}
          </Button>
        </div>
      </div>

      {form.authMethod === "password" && (
        <>
          <div>
            <Label htmlFor="conn-password" className="mb-1.5 block">
              {t("connection.password")}
            </Label>
            <Input
              id="conn-password"
              type="password"
              value={form.password}
              onChange={(e) => setForm((current) => ({ ...current, password: e.target.value }))}
              placeholder="password"
            />
          </div>
          <div className="flex items-center gap-2 rounded-md border px-3 py-2">
            <Checkbox
              id="conn-remember-password"
              checked={form.rememberPassword}
              onCheckedChange={(checked) =>
                setForm((current) => ({ ...current, rememberPassword: checked }))
              }
            />
            <Label htmlFor="conn-remember-password" className="text-sm font-normal">
              {t("connection.rememberPassword")}
            </Label>
          </div>
        </>
      )}

      {form.authMethod === "key" && (
        <>
          <div>
            <Label htmlFor="conn-key-path" className="mb-1.5 block">
              {t("ssh.privateKeyPath")}
            </Label>
            <div className="flex gap-2">
              <Input
                id="conn-key-path"
                value={form.privateKeyPath}
                onChange={(e) =>
                  setForm((current) => ({ ...current, privateKeyPath: e.target.value }))
                }
                placeholder="~/.ssh/id_rsa"
                className="flex-1"
              />
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={async () => {
                  const selected = await openFileDialog({
                    multiple: false,
                  }).catch(() => null)
                  if (selected && typeof selected === "string") {
                    setForm((current) => ({ ...current, privateKeyPath: selected }))
                  }
                }}
              >
                {t("ssh.browseKey")}
              </Button>
            </div>
          </div>
          <div>
            <Label htmlFor="conn-key-pass" className="mb-1.5 block">
              {t("ssh.privateKeyPassphrase")}
            </Label>
            <Input
              id="conn-key-pass"
              type="password"
              value={form.privateKeyPassphrase}
              onChange={(e) =>
                setForm((current) => ({ ...current, privateKeyPassphrase: e.target.value }))
              }
              placeholder="passphrase"
            />
          </div>
        </>
      )}
    </>
  )
}
