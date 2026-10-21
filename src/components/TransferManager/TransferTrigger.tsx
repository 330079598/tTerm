import React from "react"
import {
  ArrowDownToLine,
  ArrowDownUp,
  ArrowUpFromLine,
  ChevronDown,
  ChevronUp,
  FileArchive,
  PackageOpen,
  Trash2,
} from "lucide-react"
import { useTranslation } from "react-i18next"

import { summarizeTransfers, type TransferKind } from "@/components/TransferManager/transferSummary"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { keepFocusOnMouseDown } from "@/lib/terminalFocus"
import { cn } from "@/lib/utils"
import type { TransferTask } from "@/types/tab"

const KIND_ICONS: Record<TransferKind, React.ComponentType<{ className?: string }>> = {
  upload: ArrowUpFromLine,
  download: ArrowDownToLine,
  delete: Trash2,
  compress: FileArchive,
  extract: PackageOpen,
  mixed: ArrowDownUp,
}

const RING_RADIUS = 9.5
const RING_LENGTH = 2 * Math.PI * RING_RADIUS

/**
 * A ring around the icon: filled to the share moved, or a turning arc when
 * no active transfer knows its size.
 */
function ProgressRing({ progress }: { progress: number | null }) {
  const filled = progress === null ? RING_LENGTH / 4 : RING_LENGTH * progress
  return (
    <svg
      viewBox="0 0 22 22"
      className={cn(
        "absolute inset-0 size-[22px] -rotate-90",
        progress === null && "motion-safe:animate-spin"
      )}
      aria-hidden="true"
    >
      <circle
        cx="11"
        cy="11"
        r={RING_RADIUS}
        fill="none"
        strokeWidth="2"
        className="stroke-muted"
      />
      <circle
        cx="11"
        cy="11"
        r={RING_RADIUS}
        fill="none"
        strokeWidth="2"
        strokeLinecap="round"
        strokeDasharray={`${filled} ${RING_LENGTH}`}
        className="stroke-current transition-[stroke-dasharray] duration-300"
      />
    </svg>
  )
}

interface TransferTriggerProps {
  /** The top-level transfers; a folder's files are summed up in their batch. */
  transfers: TransferTask[]
  isOpen: boolean
  onToggle: () => void
}

/** The title bar button that opens the transfers panel and sums up what is moving. */
export const TransferTrigger: React.FC<TransferTriggerProps> = ({
  transfers,
  isOpen,
  onToggle,
}) => {
  const { t } = useTranslation()
  const summary = summarizeTransfers(transfers)
  const Icon = KIND_ICONS[summary.kind]
  const isActive = summary.activeCount > 0
  const percent = summary.progress === null ? null : Math.floor(summary.progress * 100)

  let label: string
  let badgeCount: number
  let badgeVariant: "default" | "secondary" | "destructive"
  if (isActive) {
    label =
      percent === null
        ? t("transfer.trigger.active", { count: summary.activeCount })
        : t("transfer.trigger.activeWithProgress", { count: summary.activeCount, percent })
    badgeCount = summary.activeCount
    badgeVariant = "default"
  } else if (summary.failedCount > 0) {
    label = t("transfer.trigger.failed", { count: summary.failedCount })
    badgeCount = summary.failedCount
    badgeVariant = "destructive"
  } else {
    label = t("transfer.trigger.idle", { count: transfers.length })
    badgeCount = transfers.length
    badgeVariant = "secondary"
  }

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="ghost"
          size="sm"
          onMouseDown={keepFocusOnMouseDown}
          onClick={onToggle}
          aria-label={label}
          aria-expanded={isOpen}
          className={cn("gap-1.5 px-2 shadow-none", isActive && "text-primary")}
        >
          {isActive ? (
            <span className="relative flex size-[22px] items-center justify-center">
              <ProgressRing progress={summary.progress} />
              <Icon className="size-3" />
            </span>
          ) : (
            <Icon className="size-4" />
          )}
          <Badge variant={badgeVariant} className="h-5 min-w-5 px-1.5">
            {badgeCount}
          </Badge>
          {isOpen ? <ChevronUp className="size-3" /> : <ChevronDown className="size-3" />}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  )
}
