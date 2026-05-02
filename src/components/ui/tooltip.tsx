import * as React from "react"
import { createPortal } from "react-dom"

import { cn } from "@/lib/utils"

type TooltipContextValue = {
  open: boolean
  setOpen: (open: boolean) => void
  triggerElement: HTMLElement | null
  setTriggerElement: (element: HTMLElement | null) => void
}

const TooltipContext = React.createContext<TooltipContextValue | null>(null)

function TooltipProvider({ children }: { children: React.ReactNode }) {
  return <>{children}</>
}

function Tooltip({ children }: { children: React.ReactNode }) {
  const [open, setOpen] = React.useState(false)
  const [triggerElement, setTriggerElement] = React.useState<HTMLElement | null>(null)
  const value = React.useMemo(
    () => ({ open, setOpen, triggerElement, setTriggerElement }),
    [open, triggerElement]
  )

  return <TooltipContext.Provider value={value}>{children}</TooltipContext.Provider>
}

type TooltipTriggerProps = React.ComponentProps<"span"> & {
  asChild?: boolean
}

function composeEventHandlers<E extends React.SyntheticEvent>(
  childHandler: ((event: E) => void) | undefined,
  ourHandler: (event: E) => void
): (event: E) => void {
  return (event: E) => {
    childHandler?.(event)
    if (!event.defaultPrevented) {
      ourHandler(event)
    }
  }
}

function TooltipTrigger({
  children,
  className,
  asChild = false,
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

  const handleBlur: React.FocusEventHandler<HTMLElement> = (event) => {
    onBlur?.(event as React.FocusEvent<HTMLSpanElement>)
    if (!event.defaultPrevented) {
      context.setOpen(false)
    }
  }

  const handleFocus: React.FocusEventHandler<HTMLElement> = (event) => {
    onFocus?.(event as React.FocusEvent<HTMLSpanElement>)
    if (!event.defaultPrevented) {
      context.setTriggerElement(event.currentTarget)
      context.setOpen(true)
    }
  }

  const handleMouseEnter: React.MouseEventHandler<HTMLElement> = (event) => {
    onMouseEnter?.(event as React.MouseEvent<HTMLSpanElement>)
    if (!event.defaultPrevented) {
      context.setTriggerElement(event.currentTarget)
      context.setOpen(true)
    }
  }

  const handleMouseLeave: React.MouseEventHandler<HTMLElement> = (event) => {
    onMouseLeave?.(event as React.MouseEvent<HTMLSpanElement>)
    if (!event.defaultPrevented) {
      context.setOpen(false)
    }
  }

  if (asChild && React.isValidElement(children)) {
    const childProps = children.props as Record<string, unknown>
    return React.cloneElement(children as React.ReactElement<Record<string, unknown>>, {
      ...props,
      onBlur: composeEventHandlers(
        childProps.onBlur as ((event: React.FocusEvent<HTMLElement>) => void) | undefined,
        handleBlur
      ),
      onFocus: composeEventHandlers(
        childProps.onFocus as ((event: React.FocusEvent<HTMLElement>) => void) | undefined,
        handleFocus
      ),
      onMouseEnter: composeEventHandlers(
        childProps.onMouseEnter as ((event: React.MouseEvent<HTMLElement>) => void) | undefined,
        handleMouseEnter
      ),
      onMouseLeave: composeEventHandlers(
        childProps.onMouseLeave as ((event: React.MouseEvent<HTMLElement>) => void) | undefined,
        handleMouseLeave
      ),
    })
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
