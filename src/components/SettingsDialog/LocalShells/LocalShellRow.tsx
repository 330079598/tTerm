import {
  ArrowDown,
  ArrowUp,
  CircleAlert,
  CopyPlus,
  Eye,
  EyeOff,
  Pencil,
  Star,
  Trash2,
} from "lucide-react"
import type { ReactNode } from "react"
import { useTranslation } from "react-i18next"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import type { ShellEntry } from "@/lib/localShells"
import { cn } from "@/lib/utils"

interface LocalShellRowProps {
  entry: ShellEntry
  isDefault: boolean
  /** One of the user's shells whose program can't be found here. */
  missing: boolean
  /** Where the user's shell sits among theirs, for moving it. */
  canMoveUp: boolean
  canMoveDown: boolean
  autoDetail: string
  onMakeDefault: () => void
  onToggleHidden: () => void
  onCopy: () => void
  onEdit: () => void
  onRemove: () => void
  onMove: (offset: -1 | 1) => void
}

/** One shell in the settings' list, with what can be done to it. */
export function LocalShellRow({
  entry,
  isDefault,
  missing,
  canMoveUp,
  canMoveDown,
  autoDetail,
  onMakeDefault,
  onToggleHidden,
  onCopy,
  onEdit,
  onRemove,
  onMove,
}: LocalShellRowProps) {
  const { t } = useTranslation()
  const action = (label: string, icon: ReactNode, onClick: () => void, disabled = false) => (
    <Button
      type="button"
      variant="ghost"
      size="icon-xs"
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={onClick}
    >
      {icon}
    </Button>
  )

  return (
    <div className={cn("flex items-center gap-3 py-2", entry.hidden && "opacity-60")}>
      <Button
        type="button"
        variant="ghost"
        size="icon-xs"
        aria-label={isDefault ? t("localShells.isDefault") : t("localShells.makeDefault")}
        title={isDefault ? t("localShells.isDefault") : t("localShells.makeDefault")}
        aria-pressed={isDefault}
        disabled={isDefault}
        className="disabled:opacity-100"
        onClick={onMakeDefault}
      >
        <Star
          className={cn(isDefault ? "fill-amber-400 text-amber-400" : "text-muted-foreground")}
        />
      </Button>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="truncate text-sm font-medium">{entry.label}</span>
          {isDefault && <Badge variant="secondary">{t("localShells.default")}</Badge>}
          {entry.kind === "detected" && (
            <Badge variant="outline">{t("localShells.detected")}</Badge>
          )}
          {entry.hidden && <Badge variant="outline">{t("localShells.hidden")}</Badge>}
          {missing && (
            <span className="text-destructive flex items-center gap-1 text-xs">
              <CircleAlert size={12} />
              {t("localShells.missing")}
            </span>
          )}
        </div>
        <div className="text-muted-foreground truncate text-xs">
          {entry.kind === "auto" ? autoDetail : entry.detail}
        </div>
      </div>
      <div className="flex shrink-0 items-center gap-0.5">
        {entry.kind === "detected" && (
          <>
            {action(t("localShells.copy"), <CopyPlus />, onCopy)}
            {entry.hidden
              ? action(t("localShells.show"), <Eye />, onToggleHidden)
              : action(t("localShells.hide"), <EyeOff />, onToggleHidden, isDefault)}
          </>
        )}
        {entry.kind === "custom" && (
          <>
            {action(t("localShells.moveUp"), <ArrowUp />, () => onMove(-1), !canMoveUp)}
            {action(t("localShells.moveDown"), <ArrowDown />, () => onMove(1), !canMoveDown)}
            {action(t("localShells.edit"), <Pencil />, onEdit)}
            {action(t("localShells.remove"), <Trash2 />, onRemove)}
          </>
        )}
      </div>
    </div>
  )
}
