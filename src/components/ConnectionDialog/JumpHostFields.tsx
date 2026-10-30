import React, { useMemo } from "react"
import { Plus, Route, Server, Trash2 } from "lucide-react"
import { useTranslation } from "react-i18next"

import { JumpHostCard } from "@/components/ConnectionDialog/JumpHostCard"
import { getJumpHostPasswordLookupKey } from "@/components/ConnectionDialog/jumpHostPayload"
import { jumpHostCandidateGroups } from "@/components/ConnectionDialog/jumpHostSource"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Label } from "@/components/ui/label"
import { Separator } from "@/components/ui/separator"
import { useConfirmDialog } from "@/components/ui/app-dialog"
import type { SavedProfile } from "@/types/tab"

import {
  ConnectionForm,
  JumpHostForm,
  createDefaultJumpHost,
} from "@/components/ConnectionDialog/types"

const MAX_JUMP_HOSTS = 8

interface JumpHostFieldsProps {
  form: ConnectionForm
  setForm: React.Dispatch<React.SetStateAction<ConnectionForm>>
  savedJumpPasswordKeys: Set<string>
  errors: Record<string, string>
  onClearError: (jumpId: string, field: string) => void
  /** Every saved connection, which hops can log in as. */
  profiles: SavedProfile[]
  /** The connection being edited, which can't be a hop on its own way. */
  currentProfileId: string
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

export const JumpHostFields: React.FC<JumpHostFieldsProps> = ({
  form,
  setForm,
  savedJumpPasswordKeys,
  errors,
  onClearError,
  profiles,
  currentProfileId,
}) => {
  const { t } = useTranslation()
  const { confirm, ConfirmDialog } = useConfirmDialog()
  const candidates = useMemo(
    () => jumpHostCandidateGroups(profiles, currentProfileId),
    [profiles, currentProfileId]
  )
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
            <JumpHostCard
              key={jump.id}
              jump={jump}
              index={index}
              count={form.jumpHosts.length}
              candidates={candidates}
              onChange={(patch) => updateJumpHost(setForm, jump.id, patch)}
              onMove={(direction) => moveJumpHost(index, direction)}
              onRemove={() => removeJumpHost(jump.id)}
              savedPasswordMasked={
                form.rememberPassword &&
                savedJumpPasswordKeys.has(getJumpHostPasswordLookupKey(jump)) &&
                jump.password.length === 0
              }
              errors={errors}
              onClearError={onClearError}
            />
          ))}
        </div>
      )}
      <ConfirmDialog />
    </div>
  )
}
