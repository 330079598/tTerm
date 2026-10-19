import React, { useCallback, useMemo, useState, useRef, useEffect, useLayoutEffect } from "react"
import { ArrowDownToLine, X, ChevronDown, ChevronUp } from "lucide-react"
import { useTranslation } from "react-i18next"

import { TransferItem } from "@/components/TransferManager/TransferItem"
import type { TransferTask } from "@/types/tab"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Spinner } from "@/components/ui/spinner"
import { keepFocusOnMouseDown, refocusTerminalIfFocusLost } from "@/lib/terminalFocus"
import { cn } from "@/lib/utils"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"

interface TransferManagerProps {
  transfers: TransferTask[]
  onCancel: (id: string) => void
  onRemove: (id: string) => void
  onRetry: (id: string) => void
  onClearCompleted: () => void
}

const PANEL_WIDTH = 420
const PANEL_MARGIN = 12

export const TransferManager: React.FC<TransferManagerProps> = ({
  transfers,
  onCancel,
  onRemove,
  onRetry,
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
        refocusTerminalIfFocusLost()
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

  const itemProps = {
    childTransfersByBatchId,
    expandedBatchIds,
    toggleBatchExpanded,
    onCancel,
    onRemove,
    onRetry,
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
        onMouseDown={keepFocusOnMouseDown}
        onClick={() => setIsOpen(!isOpen)}
        className={cn("gap-1.5 px-2 shadow-none", hasActive && "text-primary")}
      >
        {hasActive ? <Spinner /> : <ArrowDownToLine className="size-4" />}
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
                    {active.map((transfer) => (
                      <TransferItem key={transfer.id} transfer={transfer} {...itemProps} />
                    ))}
                  </div>
                )}

                {completed.length > 0 && (
                  <div className="space-y-2">
                    <div className="text-muted-foreground px-2 text-xs font-semibold tracking-wide uppercase">
                      {t("transfer.history", { defaultValue: "History" })} ({completed.length})
                    </div>
                    {completed.map((transfer) => (
                      <TransferItem key={transfer.id} transfer={transfer} {...itemProps} />
                    ))}
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
