import React, { useCallback, useMemo, useState, useRef, useEffect } from "react"
import {
  ArrowDownToLine,
  ArrowUpFromLine,
  CheckCircle2,
  Clock,
  Loader2,
  Trash2,
  X,
  XCircle,
  ChevronDown,
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
  const [isOpen, setIsOpen] = useState(false)
  const dropdownRef = useRef<HTMLDivElement>(null)

  const { active, completed } = useMemo(() => {
    const active = transfers.filter((t) => t.status === "pending" || t.status === "transferring")
    const completed = transfers.filter(
      (t) => t.status === "completed" || t.status === "failed" || t.status === "cancelled"
    )
    return { active, completed }
  }, [transfers])

  // Close dropdown when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setIsOpen(false)
      }
    }

    if (isOpen) {
      document.addEventListener("mousedown", handleClickOutside)
      return () => document.removeEventListener("mousedown", handleClickOutside)
    }
  }, [isOpen])

  const renderTransfer = useCallback(
    (transfer: TransferTask) => {
      const progress = transfer.fileSize > 0 ? (transfer.transferred / transfer.fileSize) * 100 : 0
      const isActive = transfer.status === "pending" || transfer.status === "transferring"
      const duration = transfer.endTime
        ? transfer.endTime - transfer.startTime
        : Date.now() - transfer.startTime

      return (
        <Card key={transfer.id} className="p-3">
          <div className="flex items-center gap-2">
            <div className="bg-muted flex size-6 shrink-0 items-center justify-center rounded">
              {transfer.direction === "upload" ? (
                <ArrowUpFromLine className="size-3.5" />
              ) : (
                <ArrowDownToLine className="size-3.5" />
              )}
            </div>

            <div className="min-w-0 flex-1">
              <div className="truncate text-xs font-medium">{transfer.fileName}</div>
              <div className="text-muted-foreground truncate font-mono text-[10px]">
                {transfer.direction === "upload" ? transfer.remotePath : transfer.localPath}
              </div>
            </div>

            <div className="flex shrink-0 items-center gap-1">
              {getStatusIcon(transfer.status)}
              {isActive ? (
                <Button
                  variant="ghost"
                  size="icon-xs"
                  onClick={() => onCancel(transfer.id)}
                  title={t("transfer.cancel", { defaultValue: "Cancel" })}
                >
                  <X className="size-3" />
                </Button>
              ) : (
                <Button
                  variant="ghost"
                  size="icon-xs"
                  onClick={() => onRemove(transfer.id)}
                  title={t("transfer.remove", { defaultValue: "Remove" })}
                >
                  <Trash2 className="size-3" />
                </Button>
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
                {transfer.speed > 0 && <span>{formatSpeed(transfer.speed)}</span>}
                <span>{progress.toFixed(1)}%</span>
              </div>
            </div>
          )}

          {transfer.status === "pending" && (
            <div className="text-muted-foreground mt-2 text-[10px]">
              {t("transfer.pending", { defaultValue: "Waiting..." })} •{" "}
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
        </Card>
      )
    },
    [onCancel, onRemove, t]
  )

  if (transfers.length === 0) {
    return null
  }

  const hasActive = active.length > 0
  const totalTransfers = transfers.length

  // Auto-open when there are active transfers and dropdown is closed
  const shouldShowBadge = hasActive || totalTransfers > 0

  return (
    <div className="relative" ref={dropdownRef}>
      {/* Dropdown Toggle Button */}
      <Button
        variant="outline"
        size="sm"
        onClick={() => setIsOpen(!isOpen)}
        className={cn("gap-1.5", hasActive && "border-primary text-primary")}
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

      {/* Dropdown Content */}
      {isOpen && (
        <Card className="absolute top-full right-0 z-50 mt-2 w-[420px] shadow-lg">
          <CardHeader className="flex-row items-center justify-between space-y-0 pb-3">
            <CardTitle className="text-sm font-semibold">
              {t("transfer.title", { defaultValue: "Transfers" })}
            </CardTitle>
            {completed.length > 0 && (
              <Button variant="ghost" size="xs" onClick={onClearCompleted}>
                {t("transfer.clearCompleted", { defaultValue: "Clear Completed" })}
              </Button>
            )}
          </CardHeader>

          <CardContent className="p-0">
            <ScrollArea className="max-h-[440px]">
              <div className="space-y-3 p-4 pt-0">
                {active.length > 0 && (
                  <div className="space-y-2">
                    <div className="text-muted-foreground px-2 text-xs font-semibold tracking-wide uppercase">
                      {t("transfer.active", { defaultValue: "Active" })} ({active.length})
                    </div>
                    {active.map(renderTransfer)}
                  </div>
                )}

                {completed.length > 0 && (
                  <div className="space-y-2">
                    <div className="text-muted-foreground px-2 text-xs font-semibold tracking-wide uppercase">
                      {t("transfer.history", { defaultValue: "History" })} ({completed.length})
                    </div>
                    {completed.map(renderTransfer)}
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
