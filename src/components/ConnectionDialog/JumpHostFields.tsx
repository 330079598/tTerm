import React from "react"
import { open as openFileDialog } from "@tauri-apps/plugin-dialog"
import { ChevronDown, ChevronRight, Plus, Server, Trash2, ArrowDown, ArrowUp } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Separator } from "@/components/ui/separator"
import { cn } from "@/lib/utils"

import {
  ConnectionForm,
  JumpHostForm,
  createDefaultJumpHost,
} from "@/components/ConnectionDialog/types"

const MAX_JUMP_HOSTS = 8

interface JumpHostFieldsProps {
  form: ConnectionForm
  setForm: React.Dispatch<React.SetStateAction<ConnectionForm>>
}

function updateJumpHost(
  setForm: React.Dispatch<React.SetStateAction<ConnectionForm>>,
  id: string,
  patch: Partial<JumpHostForm>
) {
  setForm((cur) => ({
    ...cur,
    jumpHosts: cur.jumpHosts.map((jump) => (jump.id === id ? { ...jump, ...patch } : jump)),
  }))
}

export const JumpHostFields: React.FC<JumpHostFieldsProps> = ({ form, setForm }) => {
  const { t } = useTranslation()

  const addJumpHost = () => {
    setForm((cur) => ({
      ...cur,
      useJumpHost: true,
      jumpHosts: [...cur.jumpHosts, createDefaultJumpHost()].slice(0, MAX_JUMP_HOSTS),
    }))
  }

  const removeJumpHost = (id: string) => {
    setForm((cur) => {
      const jumpHosts = cur.jumpHosts.filter((jump) => jump.id !== id)
      return {
        ...cur,
        useJumpHost: jumpHosts.length > 0,
        jumpHosts,
      }
    })
  }

  const moveJumpHost = (index: number, direction: -1 | 1) => {
    setForm((cur) => {
      const target = index + direction
      if (target < 0 || target >= cur.jumpHosts.length) return cur
      const jumpHosts = [...cur.jumpHosts]
      const [item] = jumpHosts.splice(index, 1)
      jumpHosts.splice(target, 0, item)
      return { ...cur, jumpHosts }
    })
  }

  return (
    <div className="space-y-3">
      <Separator />

      <div className="flex items-center gap-2">
        <Checkbox
          id="use-jump-host"
          checked={form.useJumpHost}
          onCheckedChange={(checked) =>
            setForm((cur) => ({
              ...cur,
              useJumpHost: !!checked,
              jumpHosts:
                checked && cur.jumpHosts.length === 0 ? [createDefaultJumpHost()] : cur.jumpHosts,
            }))
          }
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
          <div className="flex items-center justify-between gap-2">
            <div>
              <p className="text-sm font-medium">{t("jumpHost.chain")}</p>
              <p className="text-muted-foreground text-xs">{t("jumpHost.chainDescription")}</p>
            </div>
            <Button
              type="button"
              size="sm"
              variant="outline"
              onClick={addJumpHost}
              disabled={form.jumpHosts.length >= MAX_JUMP_HOSTS}
            >
              <Plus size={14} />
              {t("jumpHost.add")}
            </Button>
          </div>

          {form.jumpHosts.map((jump, index) => (
            <div
              key={jump.id}
              className="border-border bg-muted/20 space-y-3 rounded-lg border p-3"
            >
              <div className="flex items-center justify-between gap-2">
                <div className="min-w-0">
                  <p className="text-sm font-medium">{t("jumpHost.hop", { index: index + 1 })}</p>
                  <p className="text-muted-foreground truncate text-xs">
                    {jump.username || "user"}@{jump.host || "host"}:{jump.port || 22}
                  </p>
                </div>
                <div className="flex gap-1">
                  <Button
                    type="button"
                    size="icon"
                    variant="ghost"
                    onClick={() => moveJumpHost(index, -1)}
                    disabled={index === 0}
                    aria-label={t("jumpHost.moveUp")}
                  >
                    <ArrowUp size={14} />
                  </Button>
                  <Button
                    type="button"
                    size="icon"
                    variant="ghost"
                    onClick={() => moveJumpHost(index, 1)}
                    disabled={index === form.jumpHosts.length - 1}
                    aria-label={t("jumpHost.moveDown")}
                  >
                    <ArrowDown size={14} />
                  </Button>
                  <Button
                    type="button"
                    size="icon"
                    variant="ghost"
                    onClick={() => removeJumpHost(jump.id)}
                    aria-label={t("jumpHost.remove")}
                  >
                    <Trash2 size={14} />
                  </Button>
                </div>
              </div>

              <div className="grid grid-cols-3 gap-3">
                <div className="col-span-2">
                  <Label htmlFor={`jump-host-${jump.id}`} className="mb-1.5 block">
                    {t("jumpHost.host")}
                  </Label>
                  <Input
                    id={`jump-host-${jump.id}`}
                    value={jump.host}
                    onChange={(e) => updateJumpHost(setForm, jump.id, { host: e.target.value })}
                    placeholder="bastion.example.com"
                  />
                </div>
                <div>
                  <Label htmlFor={`jump-port-${jump.id}`} className="mb-1.5 block">
                    {t("connection.port")}
                  </Label>
                  <Input
                    id={`jump-port-${jump.id}`}
                    type="number"
                    min={1}
                    max={65535}
                    value={jump.port}
                    onChange={(e) =>
                      updateJumpHost(setForm, jump.id, { port: Number(e.target.value) })
                    }
                  />
                </div>
              </div>

              <div>
                <Label htmlFor={`jump-username-${jump.id}`} className="mb-1.5 block">
                  {t("connection.username")}
                </Label>
                <Input
                  id={`jump-username-${jump.id}`}
                  value={jump.username}
                  onChange={(e) => updateJumpHost(setForm, jump.id, { username: e.target.value })}
                  placeholder="username"
                />
              </div>

              <div>
                <Label className="mb-1.5 block">{t("ssh.authMethod")}</Label>
                <div className="flex gap-2">
                  <Button
                    type="button"
                    variant={jump.authMethod === "password" ? "default" : "outline"}
                    className={cn(
                      "flex-1",
                      jump.authMethod === "password" ? "shadow-none" : "text-muted-foreground"
                    )}
                    onClick={() => updateJumpHost(setForm, jump.id, { authMethod: "password" })}
                  >
                    {t("ssh.password")}
                  </Button>
                  <Button
                    type="button"
                    variant={jump.authMethod === "key" ? "default" : "outline"}
                    className={cn(
                      "flex-1",
                      jump.authMethod === "key" ? "shadow-none" : "text-muted-foreground"
                    )}
                    onClick={() => updateJumpHost(setForm, jump.id, { authMethod: "key" })}
                  >
                    {t("ssh.sshKey")}
                  </Button>
                </div>
              </div>

              {jump.authMethod === "password" && (
                <div>
                  <Label htmlFor={`jump-password-${jump.id}`} className="mb-1.5 block">
                    {t("connection.password")}
                  </Label>
                  <Input
                    id={`jump-password-${jump.id}`}
                    type="password"
                    value={jump.password}
                    onChange={(e) => updateJumpHost(setForm, jump.id, { password: e.target.value })}
                    placeholder="password"
                  />
                </div>
              )}

              {jump.authMethod === "key" && (
                <>
                  <div>
                    <Label htmlFor={`jump-key-path-${jump.id}`} className="mb-1.5 block">
                      {t("ssh.privateKeyPath")}
                    </Label>
                    <div className="flex gap-2">
                      <Input
                        id={`jump-key-path-${jump.id}`}
                        value={jump.privateKeyPath}
                        onChange={(e) =>
                          updateJumpHost(setForm, jump.id, { privateKeyPath: e.target.value })
                        }
                        placeholder="~/.ssh/id_rsa"
                        className="flex-1"
                      />
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        onClick={async () => {
                          const selected = await openFileDialog({ multiple: false }).catch(
                            () => null
                          )
                          if (selected && typeof selected === "string") {
                            updateJumpHost(setForm, jump.id, { privateKeyPath: selected })
                          }
                        }}
                      >
                        {t("ssh.browseKey")}
                      </Button>
                    </div>
                  </div>
                  <div>
                    <Label htmlFor={`jump-key-pass-${jump.id}`} className="mb-1.5 block">
                      {t("ssh.privateKeyPassphrase")}
                    </Label>
                    <Input
                      id={`jump-key-pass-${jump.id}`}
                      type="password"
                      value={jump.privateKeyPassphrase}
                      onChange={(e) =>
                        updateJumpHost(setForm, jump.id, { privateKeyPassphrase: e.target.value })
                      }
                      placeholder="passphrase"
                    />
                  </div>
                </>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
