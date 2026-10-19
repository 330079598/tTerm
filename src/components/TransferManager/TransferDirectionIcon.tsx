import React from "react"
import {
  ArrowDownToLine,
  ArrowUpFromLine,
  CheckCircle2,
  Clock,
  Trash2,
  XCircle,
} from "lucide-react"
import { useTranslation } from "react-i18next"

import { cn } from "@/lib/utils"
import type { TransferDirection, TransferTask } from "@/types/tab"

// Fixed hues rather than the theme's primary, which is green in one theme and
// would then read as "completed".
const DIRECTION_STYLES: Record<TransferDirection, string> = {
  upload: "bg-sky-500/15 text-sky-600 dark:text-sky-400",
  download: "bg-violet-500/15 text-violet-600 dark:text-violet-400",
  delete: "bg-muted text-muted-foreground",
}

const DIRECTION_ICONS: Record<TransferDirection, React.ComponentType<{ className?: string }>> = {
  upload: ArrowUpFromLine,
  download: ArrowDownToLine,
  delete: Trash2,
}

/** The badge on the icon's corner; a running transfer has none, its progress bar says it. */
function StatusBadge({ transfer }: { transfer: TransferTask }) {
  const className = "bg-card absolute -right-1 -bottom-1 size-3 rounded-full"
  switch (transfer.status) {
    case "pending":
      return <Clock className={cn(className, "text-muted-foreground")} />
    case "completed":
      return (
        <CheckCircle2
          className={cn(className, transfer.skipped ? "text-muted-foreground" : "text-green-500")}
        />
      )
    case "failed":
      return <XCircle className={cn(className, "text-destructive")} />
    case "cancelled":
      return <XCircle className={cn(className, "text-muted-foreground")} />
    case "transferring":
      return null
  }
}

/** A transfer's direction, tinted per direction, with its status on the corner. */
export const TransferDirectionIcon: React.FC<{ transfer: TransferTask }> = ({ transfer }) => {
  const { t } = useTranslation()
  const Icon = DIRECTION_ICONS[transfer.direction]
  const direction = t(`transfer.direction.${transfer.direction}`)
  const status =
    transfer.status === "completed" && transfer.skipped
      ? t("transfer.skippedShort")
      : t(`transfer.status.${transfer.status}`)

  return (
    <div
      role="img"
      aria-label={t("transfer.directionStatus", { direction, status })}
      title={t("transfer.directionStatus", { direction, status })}
      className={cn(
        "relative flex size-6 shrink-0 items-center justify-center rounded",
        DIRECTION_STYLES[transfer.direction]
      )}
    >
      <Icon className="size-3.5" />
      <StatusBadge transfer={transfer} />
    </div>
  )
}
