import React from "react"
import { open as openFileDialog } from "@tauri-apps/plugin-dialog"
import { Plus, Route, Server, Trash2, ArrowDown, ArrowUp } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Separator } from "@/components/ui/separator"
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip"
import { useConfirmDialog } from "@/components/ui/app-dialog"
import { cn } from "@/lib/utils"

import {
  ConnectionForm,
  JumpHostForm,
  createDefaultJumpHost,
} from "@/components/ConnectionDialog/types"

const MAX_JUMP_HOSTS = 8
const SAVED_PASSWORD_MASK = "********"

interface JumpHostFieldsProps {
  form: ConnectionForm
  setForm: React.Dispatch<React.SetStateAction<ConnectionForm>>
  savedJumpPasswordKeys: Set<string>
  errors: Record<string, string>
  onClearError: (jumpId: string, field: string) => void
}

const errorKey = (jumpId: string, field: string) => `${jumpId}:${field}`

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

function getJumpHostPasswordLookupKey(jump: Pick<JumpHostForm, "host" | "port" | "username">) {
  return `${jump.host.trim()}:${jump.port}:${jump.username.trim()}`
}

export const JumpHostFields: React.FC<JumpHostFieldsProps> = ({
  form,
  setForm,
  savedJumpPasswordKeys,
  errors,
  onClearError,
}) => {
  const { t } = useTranslation()
  const { confirm, ConfirmDialog } = useConfirmDialog()
  const target = `${form.username || "user"}@${form.host || "target"}:${form.port || 22}`

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

  const clearJumpHosts = async () => {
    const confirmed = await confirm({
      title: t("jumpHost.clear"),
      description: t("jumpHost.clearConfirm"),
      confirmText: t("jumpHost.clear"),
      cancelText: t("common.cancel"),
      variant: "destructive",
    })
    if (!confirmed) return

    setForm((current) => ({ ...current, useJumpHost: false, jumpHosts: [] }))
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
        {form.jumpHosts.length > 0 && (
          <Button
            type="button"
            size="sm"
            variant="destructive"
            className="ml-auto"
            onClick={() => void clearJumpHosts()}
          >
            <Trash2 size={14} />
            {t("jumpHost.clear")}
          </Button>
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

          <div
            className="bg-muted/35 text-muted-foreground flex items-start gap-2 rounded-md px-3 py-2 text-xs"
            role="group"
            aria-label={t("jumpHost.route", { defaultValue: "Connection route" })}
          >
            <Route size={14} className="mt-0.5 shrink-0" aria-hidden="true" />
            <span className="font-mono leading-5 break-all">
              {[
                ...form.jumpHosts.map(
                  (jump) => `${jump.username || "user"}@${jump.host || "host"}:${jump.port || 22}`
                ),
                target,
              ].join(" -> ")}
            </span>
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
                <TooltipProvider>
                  <div className="flex gap-1">
                    <Tooltip>
                      <TooltipTrigger asChild>
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
                      </TooltipTrigger>
                      <TooltipContent>{t("jumpHost.moveUp")}</TooltipContent>
                    </Tooltip>
                    <Tooltip>
                      <TooltipTrigger asChild>
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
                      </TooltipTrigger>
                      <TooltipContent>{t("jumpHost.moveDown")}</TooltipContent>
                    </Tooltip>
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <Button
                          type="button"
                          size="icon"
                          variant="ghost"
                          onClick={() => removeJumpHost(jump.id)}
                          aria-label={t("jumpHost.remove")}
                        >
                          <Trash2 size={14} />
                        </Button>
                      </TooltipTrigger>
                      <TooltipContent>{t("jumpHost.remove")}</TooltipContent>
                    </Tooltip>
                  </div>
                </TooltipProvider>
              </div>

              <div className="grid grid-cols-3 gap-3">
                <div className="col-span-2">
                  <Label htmlFor={`jump-host-${jump.id}`} className="mb-1.5 block">
                    {t("jumpHost.host")}
                  </Label>
                  <Input
                    id={`jump-host-${jump.id}`}
                    value={jump.host}
                    onChange={(e) => {
                      onClearError(jump.id, "host")
                      updateJumpHost(setForm, jump.id, { host: e.target.value })
                    }}
                    placeholder="bastion.example.com"
                    aria-invalid={!!errors[errorKey(jump.id, "host")]}
                    aria-describedby={
                      errors[errorKey(jump.id, "host")] ? `jump-host-error-${jump.id}` : undefined
                    }
                  />
                  {errors[errorKey(jump.id, "host")] && (
                    <p
                      id={`jump-host-error-${jump.id}`}
                      className="text-destructive mt-1 text-xs"
                      role="alert"
                    >
                      {errors[errorKey(jump.id, "host")]}
                    </p>
                  )}
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
                    onChange={(e) => {
                      onClearError(jump.id, "port")
                      updateJumpHost(setForm, jump.id, { port: Number(e.target.value) })
                    }}
                    aria-invalid={!!errors[errorKey(jump.id, "port")]}
                    aria-describedby={
                      errors[errorKey(jump.id, "port")] ? `jump-port-error-${jump.id}` : undefined
                    }
                  />
                  {errors[errorKey(jump.id, "port")] && (
                    <p
                      id={`jump-port-error-${jump.id}`}
                      className="text-destructive mt-1 text-xs"
                      role="alert"
                    >
                      {errors[errorKey(jump.id, "port")]}
                    </p>
                  )}
                </div>
              </div>

              <div>
                <Label htmlFor={`jump-username-${jump.id}`} className="mb-1.5 block">
                  {t("connection.username")}
                </Label>
                <Input
                  id={`jump-username-${jump.id}`}
                  value={jump.username}
                  onChange={(e) => {
                    onClearError(jump.id, "username")
                    updateJumpHost(setForm, jump.id, { username: e.target.value })
                  }}
                  placeholder="username"
                  aria-invalid={!!errors[errorKey(jump.id, "username")]}
                  aria-describedby={
                    errors[errorKey(jump.id, "username")]
                      ? `jump-username-error-${jump.id}`
                      : undefined
                  }
                />
                {errors[errorKey(jump.id, "username")] && (
                  <p
                    id={`jump-username-error-${jump.id}`}
                    className="text-destructive mt-1 text-xs"
                    role="alert"
                  >
                    {errors[errorKey(jump.id, "username")]}
                  </p>
                )}
              </div>

              <div>
                <Label className="mb-1.5 block">{t("ssh.authMethod")}</Label>
                <div className="flex gap-2" aria-label={t("ssh.authMethod")}>
                  <Button
                    type="button"
                    variant={jump.authMethod === "password" ? "default" : "outline"}
                    className={cn(
                      "flex-1",
                      jump.authMethod === "password" ? "shadow-none" : "text-muted-foreground"
                    )}
                    onClick={() => updateJumpHost(setForm, jump.id, { authMethod: "password" })}
                    aria-pressed={jump.authMethod === "password"}
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
                    aria-pressed={jump.authMethod === "key"}
                  >
                    {t("ssh.sshKey")}
                  </Button>
                </div>
              </div>

              {jump.authMethod === "password" && (
                <div>
                  {(() => {
                    const savedPasswordMasked =
                      form.rememberPassword &&
                      savedJumpPasswordKeys.has(getJumpHostPasswordLookupKey(jump)) &&
                      jump.password.length === 0
                    return (
                      <>
                        <div className="mb-1.5 flex items-center gap-2">
                          <Label htmlFor={`jump-password-${jump.id}`}>
                            {t("connection.password")}
                          </Label>
                          {savedPasswordMasked && (
                            <Badge variant="secondary" className="h-5 px-1.5 text-[10px]">
                              {t("connection.savedPasswordBadge")}
                            </Badge>
                          )}
                        </div>
                        <Input
                          id={`jump-password-${jump.id}`}
                          type="password"
                          value={savedPasswordMasked ? SAVED_PASSWORD_MASK : jump.password}
                          onChange={(e) => {
                            const value = e.target.value
                            if (savedPasswordMasked && /^\**$/.test(value)) {
                              setForm((current) => ({
                                ...current,
                                jumpHosts: current.jumpHosts.map((currentJump) =>
                                  currentJump.id === jump.id
                                    ? { ...currentJump, password: "" }
                                    : currentJump
                                ),
                              }))
                              return
                            }

                            const nextValue =
                              savedPasswordMasked && value.startsWith(SAVED_PASSWORD_MASK)
                                ? value.slice(SAVED_PASSWORD_MASK.length)
                                : value
                            updateJumpHost(setForm, jump.id, { password: nextValue })
                          }}
                          placeholder={
                            savedPasswordMasked
                              ? t("connection.savedPasswordPlaceholder")
                              : "password"
                          }
                        />
                        {savedPasswordMasked && (
                          <p className="text-muted-foreground mt-1 text-xs">
                            {t("connection.savedPasswordHint")}
                          </p>
                        )}
                      </>
                    )
                  })()}
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
                        onChange={(e) => {
                          onClearError(jump.id, "privateKeyPath")
                          updateJumpHost(setForm, jump.id, { privateKeyPath: e.target.value })
                        }}
                        placeholder="~/.ssh/id_rsa"
                        className="flex-1"
                        aria-invalid={!!errors[errorKey(jump.id, "privateKeyPath")]}
                        aria-describedby={
                          errors[errorKey(jump.id, "privateKeyPath")]
                            ? `jump-key-error-${jump.id}`
                            : undefined
                        }
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
                            onClearError(jump.id, "privateKeyPath")
                            updateJumpHost(setForm, jump.id, { privateKeyPath: selected })
                          }
                        }}
                      >
                        {t("ssh.browseKey")}
                      </Button>
                    </div>
                    {errors[errorKey(jump.id, "privateKeyPath")] && (
                      <p
                        id={`jump-key-error-${jump.id}`}
                        className="text-destructive mt-1 text-xs"
                        role="alert"
                      >
                        {errors[errorKey(jump.id, "privateKeyPath")]}
                      </p>
                    )}
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
      <ConfirmDialog />
    </div>
  )
}
