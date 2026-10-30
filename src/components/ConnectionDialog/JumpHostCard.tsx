import React from "react"
import { ArrowDown, ArrowUp, Trash2 } from "lucide-react"
import { useTranslation } from "react-i18next"

import { JumpHostManualFields } from "@/components/ConnectionDialog/JumpHostManualFields"
import { JumpHostSourceField } from "@/components/ConnectionDialog/JumpHostSourceField"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip"
import type { JumpHostCandidateGroup } from "@/components/ConnectionDialog/jumpHostSource"
import type { JumpHostForm } from "@/components/ConnectionDialog/types"

interface JumpHostCardProps {
  jump: JumpHostForm
  index: number
  count: number
  /** The saved connections a hop can log in as. */
  candidates: JumpHostCandidateGroup[]
  onChange: (patch: Partial<JumpHostForm>) => void
  onMove: (direction: -1 | 1) => void
  onRemove: () => void
  savedPasswordMasked: boolean
  errors: Record<string, string>
  onClearError: (jumpId: string, field: string) => void
}

/** One hop of the jump chain: where it goes, how it logs in, and its place in the chain. */
export const JumpHostCard: React.FC<JumpHostCardProps> = ({
  jump,
  index,
  count,
  candidates,
  onChange,
  onMove,
  onRemove,
  savedPasswordMasked,
  errors,
  onClearError,
}) => {
  const { t } = useTranslation()
  const actions = [
    {
      label: t("jumpHost.moveUp"),
      icon: ArrowUp,
      onClick: () => onMove(-1),
      disabled: index === 0,
    },
    {
      label: t("jumpHost.moveDown"),
      icon: ArrowDown,
      onClick: () => onMove(1),
      disabled: index === count - 1,
    },
    { label: t("jumpHost.remove"), icon: Trash2, onClick: onRemove, disabled: false },
  ]

  return (
    <div className="border-border bg-muted/20 space-y-3 rounded-lg border p-3">
      <div className="flex items-center justify-between gap-2">
        <div className="min-w-0">
          <p className="text-sm font-medium">{t("jumpHost.hop", { index: index + 1 })}</p>
          <p className="text-muted-foreground truncate text-xs">
            {jump.username || "user"}@{jump.host || "host"}:{jump.port || 22}
          </p>
        </div>
        <TooltipProvider>
          <div className="flex gap-1">
            {actions.map((action) => (
              <Tooltip key={action.label}>
                <TooltipTrigger asChild>
                  <Button
                    type="button"
                    size="icon"
                    variant="ghost"
                    onClick={action.onClick}
                    disabled={action.disabled}
                    aria-label={action.label}
                  >
                    <action.icon size={14} />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>{action.label}</TooltipContent>
              </Tooltip>
            ))}
          </div>
        </TooltipProvider>
      </div>

      <JumpHostSourceField jump={jump} candidates={candidates} onChange={onChange} />

      {!jump.profileId && (
        <JumpHostManualFields
          jump={jump}
          onChange={onChange}
          savedPasswordMasked={savedPasswordMasked}
          errors={errors}
          onClearError={onClearError}
        />
      )}
    </div>
  )
}
