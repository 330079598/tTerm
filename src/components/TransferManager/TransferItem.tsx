import React from "react"
import {
  ArrowDownToLine,
  ArrowUpFromLine,
  ChevronDown,
  ChevronRight,
  FileX2,
  RotateCcw,
  Trash2,
  X,
} from "lucide-react"
import { useTranslation } from "react-i18next"

import {
  formatBytes,
  formatDuration,
  formatItems,
  getStatusIcon,
} from "@/components/TransferManager/transferDisplay"
import { TransferProgress } from "@/components/TransferManager/TransferProgress"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { cn } from "@/lib/utils"
import type { TransferTask } from "@/types/tab"

interface TransferItemProps {
  transfer: TransferTask
  nested?: boolean
  childTransfersByBatchId: Map<string, TransferTask[]>
  expandedBatchIds: Set<string>
  toggleBatchExpanded: (batchId: string) => void
  onCancel: (id: string) => void
  onRemove: (id: string) => void
  onRetry: (id: string) => void
}

/** One transfer in the panel, with a folder's files below it once expanded. */
export const TransferItem: React.FC<TransferItemProps> = (props) => {
  const {
    transfer,
    nested = false,
    childTransfersByBatchId,
    expandedBatchIds,
    toggleBatchExpanded,
    onCancel,
    onRemove,
    onRetry,
  } = props
  const { t } = useTranslation()
  const isActive = transfer.status === "pending" || transfer.status === "transferring"
  const isDelete = transfer.direction === "delete"
  const duration = transfer.endTime ? transfer.endTime - transfer.startTime : 0
  const childTransfers = childTransfersByBatchId.get(transfer.id) ?? []
  const canExpand =
    (transfer.direction === "upload" || transfer.direction === "download") &&
    childTransfers.length > 0
  const isExpanded = expandedBatchIds.has(transfer.id)
  const completedChildren = childTransfers.filter(
    (child) => child.status === "completed" && !child.skipped
  ).length
  const skippedChildren = childTransfers.filter(
    (child) => child.status === "completed" && child.skipped
  ).length
  const failedChildren = childTransfers.filter((child) => child.status === "failed").length
  const cancelledChildren = childTransfers.filter((child) => child.status === "cancelled").length
  const canRetry =
    !isActive &&
    Boolean(transfer.retry) &&
    (transfer.status === "failed" || transfer.status === "cancelled")

  return (
    <Card className={cn("p-3", nested && "bg-muted/30 py-2 shadow-none")}>
      <div className="flex items-center gap-2">
        {canExpand ? (
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon-xs"
                className="size-5 shrink-0"
                onClick={() => toggleBatchExpanded(transfer.id)}
                aria-label={
                  isExpanded
                    ? t("transfer.collapseFolder", { defaultValue: "Collapse folder" })
                    : t("transfer.expandFolder", { defaultValue: "Expand folder" })
                }
                aria-expanded={isExpanded}
              >
                {isExpanded ? (
                  <ChevronDown className="size-3" />
                ) : (
                  <ChevronRight className="size-3" />
                )}
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              {isExpanded
                ? t("transfer.collapseFolder", { defaultValue: "Collapse folder" })
                : t("transfer.expandFolder", { defaultValue: "Expand folder" })}
            </TooltipContent>
          </Tooltip>
        ) : nested ? (
          <div className="w-5 shrink-0" />
        ) : null}

        <div className="bg-muted flex size-6 shrink-0 items-center justify-center rounded">
          {transfer.direction === "upload" ? (
            <ArrowUpFromLine className="size-3.5" />
          ) : transfer.direction === "delete" ? (
            <FileX2 className="size-3.5" />
          ) : (
            <ArrowDownToLine className="size-3.5" />
          )}
        </div>

        <div className="min-w-0 flex-1">
          <div className="truncate text-xs font-medium">{transfer.fileName}</div>
          <div className="text-muted-foreground truncate font-mono text-[10px]">
            {transfer.direction === "upload"
              ? transfer.remotePath
              : transfer.direction === "delete"
                ? transfer.remotePath
                : transfer.localPath}
          </div>
        </div>

        <div className="flex shrink-0 items-center gap-1">
          {getStatusIcon(transfer.status)}
          {isActive ? (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button variant="ghost" size="icon-xs" onClick={() => onCancel(transfer.id)}>
                  <X className="size-3" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>{t("transfer.cancel", { defaultValue: "Cancel" })}</TooltipContent>
            </Tooltip>
          ) : (
            <>
              {canRetry && (
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon-xs"
                      onClick={() => onRetry(transfer.id)}
                      aria-label={t("transfer.retry", { defaultValue: "Resume / Retry" })}
                    >
                      <RotateCcw className="size-3" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>
                    {t("transfer.retry", { defaultValue: "Resume / Retry" })}
                  </TooltipContent>
                </Tooltip>
              )}
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button variant="ghost" size="icon-xs" onClick={() => onRemove(transfer.id)}>
                    <Trash2 className="size-3" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>{t("transfer.remove", { defaultValue: "Remove" })}</TooltipContent>
              </Tooltip>
            </>
          )}
        </div>
      </div>

      {transfer.status === "transferring" && <TransferProgress transfer={transfer} />}

      {transfer.status === "pending" && (
        <div className="text-muted-foreground mt-2 text-[10px]">
          {t("transfer.pending", { defaultValue: "Waiting..." })} {" - "}
          {isDelete ? formatItems(transfer.fileSize, t) : formatBytes(transfer.fileSize)}
        </div>
      )}

      {transfer.status === "completed" && (
        <div className="text-muted-foreground mt-2 flex items-center gap-3 text-[10px]">
          {transfer.skipped ? (
            <span>{t("transfer.skipped", { defaultValue: "Skipped (already exists)" })}</span>
          ) : (
            <span className="text-green-600 dark:text-green-400">
              {t("transfer.completed", { defaultValue: "Completed" })}
            </span>
          )}
          <span>{formatDuration(duration)}</span>
          <span>
            {isDelete
              ? formatItems(transfer.fileSize, t)
              : formatBytes(transfer.sizeUnknown ? transfer.transferred : transfer.fileSize)}
          </span>
        </div>
      )}

      {transfer.status === "failed" && (
        <div className="text-destructive mt-2 text-[10px]">
          {transfer.error || t("transfer.failed", { defaultValue: "Failed" })}
        </div>
      )}

      {transfer.status === "cancelled" && (
        <div className="text-muted-foreground mt-2 text-[10px]">
          {t("transfer.cancelled", { defaultValue: "Cancelled" })}
        </div>
      )}

      {canExpand && (
        <button
          type="button"
          className="text-muted-foreground hover:text-foreground mt-2 flex w-full items-center justify-between rounded px-1 py-1 text-left text-[10px] transition-colors"
          onClick={() => toggleBatchExpanded(transfer.id)}
          aria-expanded={isExpanded}
        >
          <span>
            {t("transfer.folderItems", {
              count: childTransfers.length,
              defaultValue: `${childTransfers.length} file(s)`,
            })}
          </span>
          <span className="flex items-center gap-2">
            {completedChildren > 0 && (
              <span className="text-green-600 dark:text-green-400">
                {completedChildren} {t("transfer.completedShort", { defaultValue: "done" })}
              </span>
            )}
            {skippedChildren > 0 && (
              <span>
                {skippedChildren} {t("transfer.skippedShort", { defaultValue: "skipped" })}
              </span>
            )}
            {failedChildren > 0 && (
              <span className="text-destructive">
                {failedChildren} {t("transfer.failedShort", { defaultValue: "failed" })}
              </span>
            )}
            {cancelledChildren > 0 && (
              <span>
                {cancelledChildren} {t("transfer.cancelledShort", { defaultValue: "cancelled" })}
              </span>
            )}
          </span>
        </button>
      )}

      {canExpand && isExpanded && (
        <div className="mt-2 space-y-2 border-l pl-3">
          {childTransfers.map((childTransfer) => (
            <TransferItem key={childTransfer.id} {...props} transfer={childTransfer} nested />
          ))}
        </div>
      )}
    </Card>
  )
}
