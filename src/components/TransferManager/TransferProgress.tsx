import React from "react"
import { useTranslation } from "react-i18next"

import {
  formatBytes,
  formatItems,
  formatItemSpeed,
  formatSpeed,
} from "@/components/TransferManager/transferDisplay"
import type { TransferTask } from "@/types/tab"

/** The progress bar and figures of a transfer that is running. */
export const TransferProgress: React.FC<{ transfer: TransferTask }> = ({ transfer }) => {
  const { t } = useTranslation()
  const progress = transfer.fileSize > 0 ? (transfer.transferred / transfer.fileSize) * 100 : 0
  const isDelete = transfer.direction === "delete"
  const speed = transfer.speed ?? 0
  const resumedFrom = transfer.resumedFrom ?? 0
  const parallelism = transfer.parallelism ?? 0

  if (transfer.sizeUnknown) {
    // Without a total there is no share to show, only that bytes are coming.
    return (
      <div className="mt-2 space-y-1.5">
        <div className="bg-muted h-1 overflow-hidden rounded-full">
          <div className="bg-primary/60 h-full w-full animate-pulse" />
        </div>
        <div className="text-muted-foreground flex items-center justify-between text-[10px]">
          <span>{formatBytes(transfer.transferred)}</span>
          {speed > 0 && <span>{formatSpeed(speed)}</span>}
          <span>{t("transfer.sizeUnknown", { defaultValue: "Size unknown" })}</span>
        </div>
      </div>
    )
  }

  return (
    <div className="mt-2 space-y-1.5">
      <div className="bg-muted h-1 overflow-hidden rounded-full">
        <div
          className="bg-primary h-full transition-all duration-300"
          style={{ width: `${progress}%` }}
        />
      </div>
      <div className="text-muted-foreground flex items-center justify-between text-[10px]">
        <span>
          {isDelete
            ? `${formatItems(transfer.transferred, t)} / ${formatItems(transfer.fileSize, t)}`
            : `${formatBytes(transfer.transferred)} / ${formatBytes(transfer.fileSize)}`}
        </span>
        {speed > 0 && <span>{isDelete ? formatItemSpeed(speed, t) : formatSpeed(speed)}</span>}
        <span className="flex items-center gap-1">
          {parallelism > 1 && <span>{parallelism}x</span>}
          {resumedFrom > 0 && (
            <span className="text-primary">
              {t("transfer.resumed", { defaultValue: "resumed" })}
            </span>
          )}
          <span>{progress.toFixed(1)}%</span>
        </span>
      </div>
    </div>
  )
}
