import React, { useEffect, useRef, useCallback } from "react"
import {
  Plus,
  X,
  Copy,
  Terminal,
  Server,
  FolderOpen,
  Zap,
  Edit,
  Palette,
  Type,
  Languages,
  Shield,
  Pin,
  PinOff,
} from "lucide-react"
import { TabContextMenuAction } from "@/types/tab"
import { cn } from "@/lib/utils"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"

interface ContextMenuProps {
  x: number
  y: number
  actions: TabContextMenuAction[]
  onAction: (action: string) => void
  onClose: () => void
}

const getActionIcon = (icon?: string) => {
  switch (icon) {
    case "plus":
      return <Plus size={14} />
    case "x":
      return <X size={14} />
    case "copy":
      return <Copy size={14} />
    case "edit":
      return <Edit size={14} />
    case "terminal":
      return <Terminal size={14} />
    case "server":
      return <Server size={14} />
    case "folder":
      return <FolderOpen size={14} />
    case "zap":
      return <Zap size={14} />
    case "palette":
      return <Palette size={14} />
    case "type":
      return <Type size={14} />
    case "languages":
      return <Languages size={14} />
    case "shield":
      return <Shield size={14} />
    case "pin":
      return <Pin size={14} />
    case "pin-off":
      return <PinOff size={14} />
    default:
      return null
  }
}

export const ContextMenu: React.FC<ContextMenuProps> = ({ x, y, actions, onAction, onClose }) => {
  const menuRef = useRef<HTMLDivElement>(null)

  const adjustPosition = useCallback(() => {
    if (!menuRef.current) return { left: x, top: y }
    const rect = menuRef.current.getBoundingClientRect()
    const adjustedX = x + rect.width > window.innerWidth ? window.innerWidth - rect.width - 10 : x
    const adjustedY =
      y + rect.height > window.innerHeight ? window.innerHeight - rect.height - 10 : y
    return { left: Math.max(10, adjustedX), top: Math.max(10, adjustedY) }
  }, [x, y])

  const [position, setPosition] = React.useState({ left: x, top: y })

  React.useLayoutEffect(() => {
    setPosition(adjustPosition())
  }, [adjustPosition])

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) onClose()
    }
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose()
    }
    document.addEventListener("mousedown", handleClickOutside)
    document.addEventListener("keydown", handleEscape)
    return () => {
      document.removeEventListener("mousedown", handleClickOutside)
      document.removeEventListener("keydown", handleEscape)
    }
  }, [onClose])

  return (
    <div
      ref={menuRef}
      style={{ position: "fixed", left: position.left, top: position.top, zIndex: 9999 }}
    >
      <Card className="bg-popover min-w-[180px] rounded-md py-1 shadow-lg">
        <CardContent className="p-1">
          {actions.map((action, index) => {
            if (action.separator) {
              return <div key={index} className="bg-border my-1 h-px" />
            }
            return (
              <Button
                key={index}
                type="button"
                variant="ghost"
                disabled={action.disabled}
                onClick={() => {
                  if (!action.disabled) {
                    onAction(action.action)
                    onClose()
                  }
                }}
                className={cn(
                  "h-auto w-full justify-start gap-2 px-3 py-1.5 text-left text-sm font-normal",
                  action.disabled && "cursor-not-allowed opacity-40"
                )}
              >
                {getActionIcon(action.icon)}
                <span>{action.label}</span>
              </Button>
            )
          })}
        </CardContent>
      </Card>
    </div>
  )
}
