import React, { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react"
import { useTranslation } from "react-i18next"
import { SendToBack, X } from "lucide-react"

import { CollapsedSetItem } from "@/components/CollapsedTabs/CollapsedSetItem"
import type { CollapsedTabs as CollapsedTabsState } from "@/components/TTermApp/useCollapsedTabs"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { ScrollArea } from "@/components/ui/scroll-area"
import { countCollapsedTabs } from "@/lib/collapsedTabs"
import { keepFocusOnMouseDown, refocusTerminalIfFocusLost } from "@/lib/terminalFocus"

const PANEL_WIDTH = 380
const PANEL_MARGIN = 12

/** The panel's offset from its button that keeps it inside the window. */
function panelLeft(trigger: DOMRect, width: number): number {
  const overflowRight = trigger.left + width - (window.innerWidth - PANEL_MARGIN)
  const left = overflowRight > 0 ? -overflowRight : 0
  const overflowLeft = trigger.left + left - PANEL_MARGIN
  return overflowLeft < 0 ? left - overflowLeft : left
}

/**
 * The title bar's button for tabs put away, OneTab style, and its panel: put
 * every tab away, or open the ones put away again.
 */
export function CollapsedTabs({ collapsed }: { collapsed: CollapsedTabsState }) {
  const { t } = useTranslation()
  const [isOpen, setIsOpen] = useState(false)
  const containerRef = useRef<HTMLDivElement>(null)
  const [panelStyle, setPanelStyle] = useState<React.CSSProperties>({ left: 0, width: PANEL_WIDTH })
  const count = countCollapsedTabs(collapsed.sets)
  const label = t("collapsedTabs.title")

  useEffect(() => {
    if (!isOpen) return
    const handleMouseDown = (event: MouseEvent) => {
      if (!containerRef.current?.contains(event.target as Node)) setIsOpen(false)
    }
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setIsOpen(false)
    }
    document.addEventListener("mousedown", handleMouseDown)
    document.addEventListener("keydown", handleKeyDown)
    return () => {
      document.removeEventListener("mousedown", handleMouseDown)
      document.removeEventListener("keydown", handleKeyDown)
      refocusTerminalIfFocusLost()
    }
  }, [isOpen])

  const updatePosition = useCallback(() => {
    const trigger = containerRef.current?.getBoundingClientRect()
    if (!trigger) return
    const width = Math.min(PANEL_WIDTH, Math.max(window.innerWidth - PANEL_MARGIN * 2, 260))
    setPanelStyle({ left: panelLeft(trigger, width), width })
  }, [])

  useLayoutEffect(() => {
    if (!isOpen) return
    updatePosition()
    window.addEventListener("resize", updatePosition)
    return () => window.removeEventListener("resize", updatePosition)
  }, [isOpen, updatePosition])

  const handleCollapseAll = () => {
    setIsOpen(false)
    void collapsed.collapseAll()
  }

  return (
    <div className="relative" ref={containerRef}>
      <button
        type="button"
        className="tab-action"
        onMouseDown={keepFocusOnMouseDown}
        onClick={() => setIsOpen((open) => !open)}
        aria-expanded={isOpen}
        aria-label={label}
        title={label}
      >
        <SendToBack size={16} />
        {count > 0 && <span className="tunnels-running-badge">{count > 99 ? "99+" : count}</span>}
      </button>

      {isOpen && (
        <Card className="absolute top-full z-50 mt-2 shadow-lg" style={panelStyle}>
          <CardHeader className="flex-row items-center justify-between space-y-0 px-4 py-3">
            <CardTitle className="text-sm font-semibold">{label}</CardTitle>
            <div className="flex items-center gap-1">
              <Button
                variant="ghost"
                size="xs"
                disabled={!collapsed.canCollapse}
                onClick={handleCollapseAll}
              >
                <SendToBack className="size-3" />
                {t("collapsedTabs.collapseAll")}
              </Button>
              <Button
                variant="ghost"
                size="icon-xs"
                onClick={() => setIsOpen(false)}
                aria-label={t("common.close")}
                title={t("common.close")}
              >
                <X className="size-3" />
              </Button>
            </div>
          </CardHeader>
          <CardContent className="border-border border-t p-0">
            {collapsed.sets.length === 0 ? (
              <p className="text-muted-foreground px-4 py-6 text-center text-xs">
                {t("collapsedTabs.empty")}
              </p>
            ) : (
              <ScrollArea className="max-h-[60vh]">
                {collapsed.sets.map((set) => (
                  <CollapsedSetItem
                    key={set.id}
                    set={set}
                    onRestoreGroup={(setId, group) => {
                      setIsOpen(false)
                      void collapsed.restoreGroup(setId, group)
                    }}
                    onRestoreSet={(target) => {
                      setIsOpen(false)
                      void collapsed.restoreSet(target)
                    }}
                    onDeleteGroup={collapsed.deleteGroup}
                    onDeleteSet={collapsed.deleteSet}
                  />
                ))}
              </ScrollArea>
            )}
          </CardContent>
        </Card>
      )}
    </div>
  )
}
