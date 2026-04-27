import * as React from "react"
import { createPortal } from "react-dom"

import { cn } from "@/lib/utils"

type TooltipContextValue = {
  open: boolean
  setOpen: (open: boolean) => void
  triggerElement: HTMLSpanElement | null
  setTriggerElement: (element: HTMLSpanElement | null) => void
}

const TooltipContext = React.createContext<TooltipContextValue | null>(null)

function TooltipProvider({ children }: { children: React.ReactNode }) {
  return <>{children}</>
}

function Tooltip({ children }: { children: React.ReactNode }) {
  const [open, setOpen] = React.useState(false)
  const [triggerElement, setTriggerElement] = React.useState<HTMLSpanElement | null>(null)
  const value = React.useMemo(
    () => ({ open, setOpen, triggerElement, setTriggerElement }),
    [open, triggerElement]
  )

  return <TooltipContext.Provider value={value}>{children}</TooltipContext.Provider>
}

type TooltipTriggerProps = React.ComponentProps<"span"> & {
  asChild?: boolean
}

function TooltipTrigger({
  children,
  className,
  asChild: _asChild = false,
  onBlur,
  onFocus,
  onMouseEnter,
  onMouseLeave,
  ...props
}: TooltipTriggerProps) {
  const context = React.useContext(TooltipContext)
  if (!context) {
    throw new Error("TooltipTrigger must be used within Tooltip")
  }

  const handleBlur: React.FocusEventHandler<HTMLSpanElement> = (event) => {
    onBlur?.(event)
    if (!event.defaultPrevented) {
      context.setOpen(false)
    }
  }

  const handleFocus: React.FocusEventHandler<HTMLSpanElement> = (event) => {
    onFocus?.(event)
    if (!event.defaultPrevented) {
      context.setTriggerElement(event.currentTarget)
      context.setOpen(true)
    }
  }

  const handleMouseEnter: React.MouseEventHandler<HTMLSpanElement> = (event) => {
    onMouseEnter?.(event)
    if (!event.defaultPrevented) {
      context.setTriggerElement(event.currentTarget)
      context.setOpen(true)
    }
  }

  const handleMouseLeave: React.MouseEventHandler<HTMLSpanElement> = (event) => {
    onMouseLeave?.(event)
    if (!event.defaultPrevented) {
      context.setOpen(false)
    }
  }

  return (
    <span
      className={cn("inline-flex", className)}
      onBlur={handleBlur}
      onFocus={handleFocus}
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
      {...props}
    >
      {children}
    </span>
  )
}

type TooltipContentProps = React.ComponentProps<"span"> & {
  side?: "top" | "bottom"
}

function TooltipContent({ children, className, side = "top", ...props }: TooltipContentProps) {
  const context = React.useContext(TooltipContext)
  const [position, setPosition] = React.useState<{ left: number; top: number } | null>(null)

  if (!context) {
    throw new Error("TooltipContent must be used within Tooltip")
  }

  React.useLayoutEffect(() => {
    if (!context.open) {
      return undefined
    }

    const updatePosition = () => {
      const rect = context.triggerElement?.getBoundingClientRect()
      if (!rect) return

      setPosition({
        left: rect.left + rect.width / 2,
        top: side === "top" ? rect.top : rect.bottom,
      })
    }

    updatePosition()
    window.addEventListener("resize", updatePosition)
    window.addEventListener("scroll", updatePosition, true)

    return () => {
      window.removeEventListener("resize", updatePosition)
      window.removeEventListener("scroll", updatePosition, true)
    }
  }, [context.open, context.triggerElement, side])

  if (!context.open || !position || typeof document === "undefined") {
    return null
  }

  return createPortal(
    <span
      role="tooltip"
      style={{ left: position.left, top: position.top }}
      className={cn(
        "bg-popover text-popover-foreground pointer-events-none fixed z-[100] w-max max-w-64 -translate-x-1/2 rounded-md border px-3 py-1.5 text-xs leading-relaxed shadow-md",
        side === "top" ? "-translate-y-[calc(100%+0.5rem)]" : "translate-y-2",
        className
      )}
      {...props}
    >
      {children}
    </span>,
    document.body
  )
}

export { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger }
