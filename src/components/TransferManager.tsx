import React, { useCallback, useMemo, useState, useRef, useEffect, useLayoutEffect } from "react"
import {
  ArrowDownToLine,
  ArrowUpFromLine,
  FileX2,
  CheckCircle2,
  Clock,
  Loader2,
  Trash2,
  X,
  XCircle,
  ChevronDown,
  ChevronRight,
  ChevronUp,
} from "lucide-react"
import { useTranslation } from "react-i18next"
import type { TransferTask, TransferStatus } from "@/types/tab"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { ScrollArea } from "@/components/ui/scroll-area"
import { cn } from "@/lib/utils"

interface TransferManagerProps {
  transfers: TransferTask[]
  onCancel: (id: string) => void
  onRemove: (id: string) => void
  onClearCompleted: () => void
}

const PANEL_WIDTH = 420
const PANEL_MARGIN = 12

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B"
  const k = 1024
  const sizes = ["B", "KB", "MB", "GB"]
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`
}

function formatSpeed(bytesPerSecond: number): string {
  return `${formatBytes(bytesPerSecond)}/s`
}

function formatDuration(ms: number): string {
  const seconds = Math.floor(ms / 1000)
  if (seconds < 60) return `${seconds}s`
  const minutes = Math.floor(seconds / 60)
  const remainingSeconds = seconds % 60
  return `${minutes}m ${remainingSeconds}s`
}

function getStatusIcon(status: TransferStatus) {
  switch (status) {
    case "pending":
      return <Clock size={16} className="text-muted-foreground" />
    case "transferring":
      return <Loader2 size={16} className="text-primary animate-spin" />
    case "completed":
      return <CheckCircle2 size={16} className="text-green-500" />
    case "failed":
      return <XCircle size={16} className="text-destructive" />
    case "cancelled":
      return <XCircle size={16} className="text-muted-foreground" />
  }
}

export const TransferManager: React.FC<TransferManagerProps> = ({
  transfers,
  onCancel,
  onRemove,
  onClearCompleted,
}) => {
  const { t } = useTranslation()
  const childTransfersByBatchId = useMemo(() => {
    const next = new Map<string, TransferTask[]>()

    for (const transfer of transfers) {
      if (!transfer.batchId) {
        continue
      }

      const children = next.get(transfer.batchId) ?? []
      children.push(transfer)
      next.set(transfer.batchId, children)
    }

    for (const children of next.values()) {
      children.sort((first, second) => first.startTime - second.startTime)
    }

    return next
  }, [transfers])
  const visibleTransfers = transfers.filter((transfer) => !transfer.batchId)
  const [expandedBatchIds, setExpandedBatchIds] = useState<Set<string>>(() => new Set())
  const [isOpen, setIsOpen] = useState(false)
  const dropdownRef = useRef<HTMLDivElement>(null)
  const [panelStyle, setPanelStyle] = useState<React.CSSProperties>({ left: 0, width: PANEL_WIDTH })

  const toggleBatchExpanded = useCallback((batchId: string) => {
    setExpandedBatchIds((current) => {
      const next = new Set(current)
      if (next.has(batchId)) {
        next.delete(batchId)
      } else {
        next.add(batchId)
      }
      return next
    })
  }, [])

  const { active, completed } = useMemo(() => {
    const active = visibleTransfers.filter(
      (t) => t.status === "pending" || t.status === "transferring"
    )
    const completed = visibleTransfers.filter(
      (t) => t.status === "completed" || t.status === "failed" || t.status === "cancelled"
    )
    return { active, completed }
  }, [visibleTransfers])

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setIsOpen(false)
      }
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setIsOpen(false)
      }
    }

    if (isOpen) {
      document.addEventListener("mousedown", handleClickOutside)
      document.addEventListener("keydown", handleKeyDown)

      return () => {
        document.removeEventListener("mousedown", handleClickOutside)
        document.removeEventListener("keydown", handleKeyDown)
      }
    }
  }, [isOpen])

  const updatePanelPosition = useCallback(() => {
    if (!dropdownRef.current) {
      return
    }

    const triggerRect = dropdownRef.current.getBoundingClientRect()
    const availableWidth = Math.max(window.innerWidth - PANEL_MARGIN * 2, 280)
    const panelWidth = Math.min(PANEL_WIDTH, availableWidth)
    let nextLeft = 0

    const overflowRight = triggerRect.left + panelWidth - (window.innerWidth - PANEL_MARGIN)
    if (overflowRight > 0) {
      nextLeft -= overflowRight
    }

    const overflowLeft = triggerRect.left + nextLeft - PANEL_MARGIN
    if (overflowLeft < 0) {
      nextLeft -= overflowLeft
    }

    setPanelStyle((prev) => {
      if (prev.left === nextLeft && prev.width === panelWidth) {
        return prev
      }

      return { left: nextLeft, width: panelWidth }
    })
  }, [])

  useLayoutEffect(() => {
    if (!isOpen) {
      return
    }

    updatePanelPosition()

    const handleResize = () => {
      updatePanelPosition()
    }

    window.addEventListener("resize", handleResize)

    return () => {
      window.removeEventListener("resize", handleResize)
    }
  }, [isOpen, updatePanelPosition])

  function renderTransfer(transfer: TransferTask, nested = false) {
    const progress = transfer.fileSize > 0 ? (transfer.transferred / transfer.fileSize) * 100 : 0
    const isActive = transfer.status === "pending" || transfer.status === "transferring"
    const speed = transfer.speed ?? 0
    const duration = transfer.endTime ? transfer.endTime - transfer.startTime : 0
    const childTransfers = childTransfersByBatchId.get(transfer.id) ?? []
    const canExpand = transfer.direction === "upload" && childTransfers.length > 0
    const isExpanded = expandedBatchIds.has(transfer.id)
    const completedChildren = childTransfers.filter((child) => child.status === "completed").length
    const failedChildren = childTransfers.filter((child) => child.status === "failed").length
    const cancelledChildren = childTransfers.filter((child) => child.status === "cancelled").length

    return (
      <Card key={transfer.id} className={cn("p-3", nested && "bg-muted/30 py-2 shadow-none")}>
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
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button variant="ghost" size="icon-xs" onClick={() => onRemove(transfer.id)}>
                    <Trash2 className="size-3" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>{t("transfer.remove", { defaultValue: "Remove" })}</TooltipContent>
              </Tooltip>
            )}
          </div>
        </div>

        {transfer.status === "transferring" && (
          <div className="mt-2 space-y-1.5">
            <div className="bg-muted h-1 overflow-hidden rounded-full">
              <div
                className="bg-primary h-full transition-all duration-300"
                style={{ width: `${progress}%` }}
              />
            </div>
            <div className="text-muted-foreground flex items-center justify-between text-[10px]">
              <span>
                {formatBytes(transfer.transferred)} / {formatBytes(transfer.fileSize)}
              </span>
              {speed > 0 && <span>{formatSpeed(speed)}</span>}
              <span>{progress.toFixed(1)}%</span>
            </div>
          </div>
        )}

        {transfer.status === "pending" && (
          <div className="text-muted-foreground mt-2 text-[10px]">
            {t("transfer.pending", { defaultValue: "Waiting..." })} {" - "}
            {formatBytes(transfer.fileSize)}
          </div>
        )}

        {transfer.status === "completed" && (
          <div className="text-muted-foreground mt-2 flex items-center gap-3 text-[10px]">
            <span className="text-green-600 dark:text-green-400">
              {t("transfer.completed", { defaultValue: "Completed" })}
            </span>
            <span>{formatDuration(duration)}</span>
            <span>{formatBytes(transfer.fileSize)}</span>
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
            {childTransfers.map((childTransfer) => renderTransfer(childTransfer, true))}
          </div>
        )}
      </Card>
    )
  }

  if (visibleTransfers.length === 0) {
    return null
  }

  const hasActive = active.length > 0
  const totalTransfers = visibleTransfers.length
  const shouldShowBadge = hasActive || totalTransfers > 0

  return (
    <div className="relative" ref={dropdownRef}>
      <Button
        variant="ghost"
        size="sm"
        onClick={() => setIsOpen(!isOpen)}
        className={cn("gap-1.5 px-2 shadow-none", hasActive && "text-primary")}
      >
        {hasActive ? (
          <Loader2 className="size-4 animate-spin" />
        ) : (
          <ArrowDownToLine className="size-4" />
        )}
        {shouldShowBadge && totalTransfers > 0 && (
          <Badge variant="default" className="h-5 min-w-5 px-1.5">
            {totalTransfers}
          </Badge>
        )}
        {isOpen ? <ChevronUp className="size-3" /> : <ChevronDown className="size-3" />}
      </Button>

      {isOpen && (
        <Card className="absolute top-full z-50 mt-2 shadow-lg" style={panelStyle}>
          <CardHeader className="flex-row items-center justify-between space-y-0 pb-3">
            <CardTitle className="text-sm font-semibold">
              {t("transfer.title", { defaultValue: "Transfers" })}
            </CardTitle>
            <div className="flex items-center gap-1">
              {completed.length > 0 && (
                <Button variant="ghost" size="xs" onClick={onClearCompleted}>
                  {t("transfer.clearCompleted", { defaultValue: "Clear Completed" })}
                </Button>
              )}
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    variant="ghost"
                    size="icon-xs"
                    onClick={() => setIsOpen(false)}
                    aria-label={t("common.close", { defaultValue: "Close" })}
                  >
                    <X className="size-3" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>{t("common.close", { defaultValue: "Close" })}</TooltipContent>
              </Tooltip>
            </div>
          </CardHeader>

          <CardContent className="p-0">
            <ScrollArea className="max-h-[440px]">
              <div className="space-y-3 p-4 pt-0">
                {active.length > 0 && (
                  <div className="space-y-2">
                    <div className="text-muted-foreground px-2 text-xs font-semibold tracking-wide uppercase">
                      {t("transfer.active", { defaultValue: "Active" })} ({active.length})
                    </div>
                    {active.map((transfer) => renderTransfer(transfer))}
                  </div>
                )}

                {completed.length > 0 && (
                  <div className="space-y-2">
                    <div className="text-muted-foreground px-2 text-xs font-semibold tracking-wide uppercase">
                      {t("transfer.history", { defaultValue: "History" })} ({completed.length})
                    </div>
                    {completed.map((transfer) => renderTransfer(transfer))}
                  </div>
                )}
              </div>
            </ScrollArea>
          </CardContent>
        </Card>
      )}
    </div>
  )
}
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
