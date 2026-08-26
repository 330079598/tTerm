import React, { useEffect, useRef, useCallback, useState } from "react"
import { createPortal } from "react-dom"
import {
  Plus,
  X,
  Copy,
  ClipboardPaste,
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
  PanelLeft,
  PanelRight,
  PanelTop,
  PanelBottom,
  Search,
  Star,
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
      return <Plus size={14} aria-hidden="true" />
    case "x":
      return <X size={14} aria-hidden="true" />
    case "copy":
      return <Copy size={14} aria-hidden="true" />
    case "paste":
      return <ClipboardPaste size={14} aria-hidden="true" />
    case "search":
      return <Search size={14} aria-hidden="true" />
    case "star":
      return <Star size={14} aria-hidden="true" />
    case "edit":
      return <Edit size={14} aria-hidden="true" />
    case "terminal":
      return <Terminal size={14} aria-hidden="true" />
    case "server":
      return <Server size={14} aria-hidden="true" />
    case "folder":
      return <FolderOpen size={14} aria-hidden="true" />
    case "zap":
      return <Zap size={14} aria-hidden="true" />
    case "palette":
      return <Palette size={14} aria-hidden="true" />
    case "type":
      return <Type size={14} aria-hidden="true" />
    case "languages":
      return <Languages size={14} aria-hidden="true" />
    case "shield":
      return <Shield size={14} aria-hidden="true" />
    case "pin":
      return <Pin size={14} aria-hidden="true" />
    case "pin-off":
      return <PinOff size={14} aria-hidden="true" />
    case "split-right":
      return <PanelRight size={14} aria-hidden="true" />
    case "split-left":
      return <PanelLeft size={14} aria-hidden="true" />
    case "split-down":
      return <PanelBottom size={14} aria-hidden="true" />
    case "split-above":
      return <PanelTop size={14} aria-hidden="true" />
    default:
      return null
  }
}

function getMenuItemIndices(actions: TabContextMenuAction[]) {
  const indices: number[] = []
  for (let i = 0; i < actions.length; i++) {
    if (!actions[i].separator && !actions[i].disabled) {
      indices.push(i)
    }
  }
  return indices
}

export const ContextMenu: React.FC<ContextMenuProps> = ({ x, y, actions, onAction, onClose }) => {
  const menuRef = useRef<HTMLDivElement>(null)
  const enabledIndices = getMenuItemIndices(actions)
  const [focusIndex, setFocusIndex] = useState(() => enabledIndices[0] ?? -1)

  const adjustPosition = useCallback(() => {
    if (!menuRef.current) return { left: x, top: y }
    const rect = menuRef.current.getBoundingClientRect()
    const adjustedX = x + rect.width > window.innerWidth ? window.innerWidth - rect.width - 10 : x
    const adjustedY =
      y + rect.height > window.innerHeight ? window.innerHeight - rect.height - 10 : y
    return { left: Math.max(10, adjustedX), top: Math.max(10, adjustedY) }
  }, [x, y])

  const [position, setPosition] = useState({ left: x, top: y })

  React.useLayoutEffect(() => {
    setPosition(adjustPosition())
  }, [adjustPosition])

  // Focus the menuitem element when focusIndex changes
  useEffect(() => {
    if (focusIndex >= 0 && menuRef.current) {
      const targetItem = menuRef.current.querySelector<HTMLElement>(
        `[role="menuitem"][data-action-index="${focusIndex}"]`
      )
      targetItem?.focus()
    }
  }, [focusIndex])

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault()
        onClose()
        return
      }

      if (enabledIndices.length === 0) return

      const currentPos = enabledIndices.indexOf(focusIndex)

      switch (e.key) {
        case "ArrowDown": {
          e.preventDefault()
          const nextPos = currentPos < enabledIndices.length - 1 ? currentPos + 1 : 0
          setFocusIndex(enabledIndices[nextPos])
          break
        }
        case "ArrowUp": {
          e.preventDefault()
          const prevPos = currentPos > 0 ? currentPos - 1 : enabledIndices.length - 1
          setFocusIndex(enabledIndices[prevPos])
          break
        }
        case "Home": {
          e.preventDefault()
          setFocusIndex(enabledIndices[0])
          break
        }
        case "End": {
          e.preventDefault()
          setFocusIndex(enabledIndices[enabledIndices.length - 1])
          break
        }
        case "Tab": {
          e.preventDefault()
          if (e.shiftKey) {
            const prevPos = currentPos > 0 ? currentPos - 1 : enabledIndices.length - 1
            setFocusIndex(enabledIndices[prevPos])
          } else {
            const nextPos = currentPos < enabledIndices.length - 1 ? currentPos + 1 : 0
            setFocusIndex(enabledIndices[nextPos])
          }
          break
        }
      }
    }

    document.addEventListener("keydown", handleKeyDown)
    return () => document.removeEventListener("keydown", handleKeyDown)
  }, [enabledIndices, focusIndex, onClose])

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) onClose()
    }
    document.addEventListener("mousedown", handleClickOutside)
    return () => document.removeEventListener("mousedown", handleClickOutside)
  }, [onClose])

  const menu = (
    <div
      ref={menuRef}
      style={{ position: "fixed", left: position.left, top: position.top, zIndex: 9999 }}
    >
      <Card
        role="menu"
        className="bg-popover w-[min(16rem,calc(100vw-20px))] min-w-52 rounded-md py-1 shadow-lg"
      >
        <CardContent className="p-1">
          {actions.map((action, index) => {
            if (action.separator) {
              return <div key={index} role="separator" className="bg-border my-1 h-px" />
            }
            const isFocused = focusIndex === index
            return (
              <Button
                key={index}
                type="button"
                variant="ghost"
                role="menuitem"
                data-action-index={index}
                disabled={action.disabled}
                tabIndex={isFocused ? 0 : -1}
                onFocus={() => setFocusIndex(index)}
                onClick={() => {
                  if (!action.disabled) {
                    onAction(action.action)
                    onClose()
                  }
                }}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault()
                    if (!action.disabled) {
                      onAction(action.action)
                      onClose()
                    }
                  }
                }}
                className={cn(
                  "h-auto w-full justify-start gap-1.5 px-2 py-1.5 text-left text-sm font-normal whitespace-normal",
                  action.disabled && "cursor-not-allowed opacity-40"
                )}
                aria-label={action.label}
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

  // Keep viewport coordinates independent from transformed ancestors such as the SFTP drawer.
  return typeof document === "undefined" ? menu : createPortal(menu, document.body)
}
