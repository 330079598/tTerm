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

type TooltipTriggerProps = Omit<
  React.ComponentProps<"span">,
  "onBlur" | "onFocus" | "onMouseEnter" | "onMouseLeave" | "onClick"
> & {
  asChild?: boolean
  onBlur?: React.FocusEventHandler<HTMLElement>
  onFocus?: React.FocusEventHandler<HTMLElement>
  onMouseEnter?: React.MouseEventHandler<HTMLElement>
  onMouseLeave?: React.MouseEventHandler<HTMLElement>
  onClick?: React.MouseEventHandler<HTMLElement>
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
  onClick,
  ...props
}: TooltipTriggerProps) {
  const context = React.useContext(TooltipContext)
  if (!context) {
    throw new Error("TooltipTrigger must be used within Tooltip")
  }

  const handleBlur: React.FocusEventHandler<HTMLElement> = (event) => {
    onBlur?.(event)
    if (!event.defaultPrevented) {
      context.setOpen(false)
    }
  }

  const handleFocus: React.FocusEventHandler<HTMLElement> = (event) => {
    onFocus?.(event)
    if (!event.defaultPrevented) {
      context.setTriggerElement(event.currentTarget)
      context.setOpen(true)
    }
  }

  const handleMouseEnter: React.MouseEventHandler<HTMLElement> = (event) => {
    onMouseEnter?.(event)
    if (!event.defaultPrevented) {
      context.setTriggerElement(event.currentTarget)
      context.setOpen(true)
    }
  }

  const handleMouseLeave: React.MouseEventHandler<HTMLElement> = (event) => {
    onMouseLeave?.(event)
    if (!event.defaultPrevented) {
      context.setOpen(false)
    }
  }

  // A click means the interaction moved from "hover" to "activate" (often
  // navigating away, e.g. connecting a profile). The trigger's mouse never
  // technically "leaves" in that case, so mouseleave never fires and the
  // portaled tooltip is left stuck open over whatever renders next.
  const handleClick: React.MouseEventHandler<HTMLElement> = (event) => {
    onClick?.(event)
    if (!event.defaultPrevented) {
      context.setOpen(false)
    }
  }

  if (asChild && React.isValidElement(children)) {
    const childProps = children.props as Record<string, unknown>
    return React.cloneElement(children as React.ReactElement<Record<string, unknown>>, {
      ...props,
      className: cn(className, childProps.className as string | undefined),
      onBlur: composeEventHandlers(
        childProps.onBlur as React.FocusEventHandler<HTMLElement> | undefined,
        handleBlur
      ),
      onFocus: composeEventHandlers(
        childProps.onFocus as React.FocusEventHandler<HTMLElement> | undefined,
        handleFocus
      ),
      onMouseEnter: composeEventHandlers(
        childProps.onMouseEnter as React.MouseEventHandler<HTMLElement> | undefined,
        handleMouseEnter
      ),
      onMouseLeave: composeEventHandlers(
        childProps.onMouseLeave as React.MouseEventHandler<HTMLElement> | undefined,
        handleMouseLeave
      ),
      onClick: composeEventHandlers(
        childProps.onClick as React.MouseEventHandler<HTMLElement> | undefined,
        handleClick
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
      onClick={handleClick}
      {...props}
    >
      {children}
    </span>
  )
}

type TooltipSide = "top" | "bottom"

type TooltipContentProps = React.ComponentProps<"span"> & {
  side?: TooltipSide
}

const TOOLTIP_VIEWPORT_PADDING = 8
/** Between the trigger and the tooltip: the 0.5rem of the translate classes. */
const TOOLTIP_GAP = 8

/**
 * The side a tooltip goes on: the one asked for, unless the tooltip would
 * leave the window there and fits on the other, as one above a tab in the
 * title bar would.
 */
function placeTooltip(side: TooltipSide, trigger: DOMRect, height: number): TooltipSide {
  const fitsAbove = trigger.top - TOOLTIP_GAP - height >= TOOLTIP_VIEWPORT_PADDING
  const fitsBelow =
    trigger.bottom + TOOLTIP_GAP + height <= window.innerHeight - TOOLTIP_VIEWPORT_PADDING
  if (side === "top") return fitsAbove || !fitsBelow ? "top" : "bottom"
  return fitsBelow || !fitsAbove ? "bottom" : "top"
}

function TooltipContent({ children, className, side = "top", ...props }: TooltipContentProps) {
  const context = React.useContext(TooltipContext)
  const contentRef = React.useRef<HTMLSpanElement>(null)
  const [position, setPosition] = React.useState<{
    left: number
    top: number
    side: TooltipSide
  } | null>(null)

  if (!context) {
    throw new Error("TooltipContent must be used within Tooltip")
  }

  const updatePosition = React.useCallback(() => {
    const rect = context.triggerElement?.getBoundingClientRect()
    if (!rect) return

    const preferredLeft = rect.left + rect.width / 2
    const content = contentRef.current?.getBoundingClientRect()
    const tooltipWidth = content?.width ?? 0
    const halfTooltipWidth = tooltipWidth / 2
    const minLeft = TOOLTIP_VIEWPORT_PADDING + halfTooltipWidth
    const maxLeft = window.innerWidth - TOOLTIP_VIEWPORT_PADDING - halfTooltipWidth
    const left = tooltipWidth
      ? Math.min(Math.max(preferredLeft, minLeft), Math.max(minLeft, maxLeft))
      : preferredLeft
    const placement = placeTooltip(side, rect, content?.height ?? 0)
    const top = placement === "top" ? rect.top : rect.bottom

    setPosition((current) =>
      current?.left === left && current.top === top && current.side === placement
        ? current
        : { left, top, side: placement }
    )
  }, [context.triggerElement, side])

  React.useLayoutEffect(() => {
    if (!context.open) {
      return undefined
    }

    updatePosition()
    window.addEventListener("resize", updatePosition)
    window.addEventListener("scroll", updatePosition, true)

    return () => {
      window.removeEventListener("resize", updatePosition)
      window.removeEventListener("scroll", updatePosition, true)
    }
  }, [context.open, updatePosition])

  React.useLayoutEffect(() => {
    if (context.open && position) {
      updatePosition()
    }
  }, [children, className, context.open, position, updatePosition])

  if (!context.open || !position || typeof document === "undefined") {
    return null
  }

  return createPortal(
    <span
      ref={contentRef}
      role="tooltip"
      style={{ left: position.left, top: position.top }}
      className={cn(
        "bg-popover text-popover-foreground pointer-events-none fixed z-[300] w-max max-w-[min(16rem,calc(100vw-1rem))] -translate-x-1/2 rounded-md border px-3 py-1.5 text-xs leading-relaxed shadow-md",
        position.side === "top" ? "-translate-y-[calc(100%+0.5rem)]" : "translate-y-2",
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
