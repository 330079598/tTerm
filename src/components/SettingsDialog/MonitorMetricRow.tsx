import React from "react"
import { useSortable } from "@dnd-kit/react/sortable"
import { GripVertical, type LucideIcon } from "lucide-react"
import { useTranslation } from "react-i18next"

import { monitorMetricDragId } from "@/components/SettingsDialog/monitorMetricOrder"
import { Checkbox } from "@/components/ui/checkbox"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import type { MonitorMetricId } from "@/lib/monitorMetrics"
import { cn } from "@/lib/utils"

interface MonitorMetricRowProps {
  checked: boolean
  icon: LucideIcon
  id: MonitorMetricId
  index: number
  isLastSelected: boolean
  label: string
  onCheckedChange: (checked: boolean) => void
}

export const MonitorMetricRow: React.FC<MonitorMetricRowProps> = ({
  checked,
  icon: Icon,
  id,
  index,
  isLastSelected,
  label,
  onCheckedChange,
}) => {
  const { t } = useTranslation()
  const { ref, handleRef, isDragging, isDropTarget } = useSortable({
    id: monitorMetricDragId(id),
    index,
    group: "monitor-status-metrics",
    disabled: !checked,
    transition: {
      duration: 180,
      easing: "cubic-bezier(0.2, 0.8, 0.2, 1)",
    },
  })
  const dragLabel = t("settings.monitorMetricDragLabel", {
    metric: label,
    defaultValue: `Drag to reorder ${label}`,
  })

  return (
    <div
      ref={ref}
      className={cn(
        "border-border bg-background flex min-h-10 items-center gap-2 rounded-md border px-2 py-2 transition-colors",
        checked ? "hover:bg-muted/50 will-change-transform" : "opacity-70",
        isDragging && "z-10 opacity-80 shadow-md",
        isDropTarget && "border-primary/70 bg-primary/5"
      )}
      role="listitem"
    >
      {checked ? (
        <Tooltip>
          <TooltipTrigger asChild>
            <button
              ref={handleRef}
              type="button"
              className="text-muted-foreground hover:bg-muted hover:text-foreground focus-visible:ring-ring/50 flex size-7 shrink-0 cursor-grab touch-none items-center justify-center rounded-sm outline-none focus-visible:ring-[3px] active:cursor-grabbing"
              aria-label={dragLabel}
            >
              <GripVertical size={15} aria-hidden="true" />
            </button>
          </TooltipTrigger>
          <TooltipContent>{dragLabel}</TooltipContent>
        </Tooltip>
      ) : (
        <span className="size-7 shrink-0" aria-hidden="true" />
      )}
      <Checkbox
        checked={checked}
        disabled={isLastSelected}
        onCheckedChange={onCheckedChange}
        aria-label={label}
      />
      <Icon size={15} className="text-muted-foreground shrink-0" aria-hidden="true" />
      <button
        type="button"
        className="min-w-0 flex-1 cursor-pointer truncate text-left text-sm"
        disabled={isLastSelected}
        onClick={() => onCheckedChange(!checked)}
      >
        {label}
      </button>
    </div>
  )
}
