import * as React from "react"
import { createPortal } from "react-dom"
import { XIcon } from "lucide-react"

import { cn } from "@/lib/utils"
import { Button } from "@/components/ui/button"

type DialogContextValue = {
  open: boolean
  setOpen: (open: boolean) => void
}

type DialogProps = {
  children: React.ReactNode
  open?: boolean
  defaultOpen?: boolean
  onOpenChange?: (open: boolean) => void
}

type DialogInteractOutsideEvent = {
  readonly defaultPrevented: boolean
  originalEvent: MouseEvent
  preventDefault: () => void
}

type SlottableButtonProps = {
  children?: React.ReactNode
  className?: string
  onClick?: React.MouseEventHandler<Element>
  [key: string]: unknown
}

const DialogContext = React.createContext<DialogContextValue | null>(null)

function useDialogContext(componentName: string) {
  const context = React.useContext(DialogContext)

  if (!context) {
    throw new Error(`${componentName} must be used within Dialog`)
  }

  return context
}

function composeRefs<T>(...refs: Array<React.Ref<T> | undefined>) {
  return (node: T | null) => {
    refs.forEach((ref) => {
      if (typeof ref === "function") {
        ref(node)
      } else if (ref) {
        ;(ref as React.MutableRefObject<T | null>).current = node
      }
    })
  }
}

function Dialog({ children, open, defaultOpen = false, onOpenChange }: DialogProps) {
  const [uncontrolledOpen, setUncontrolledOpen] = React.useState(defaultOpen)
  const isControlled = open !== undefined
  const resolvedOpen = isControlled ? open : uncontrolledOpen

  const setOpen = React.useCallback(
    (nextOpen: boolean) => {
      if (!isControlled) {
        setUncontrolledOpen(nextOpen)
      }

      onOpenChange?.(nextOpen)
    },
    [isControlled, onOpenChange]
  )

  const value = React.useMemo(() => ({ open: resolvedOpen, setOpen }), [resolvedOpen, setOpen])

  return <DialogContext.Provider value={value}>{children}</DialogContext.Provider>
}

function DialogTrigger({
  children,
  asChild = false,
  onClick,
  ...props
}: React.ButtonHTMLAttributes<HTMLButtonElement> & {
  asChild?: boolean
  children?: React.ReactNode
}) {
  const { setOpen } = useDialogContext("DialogTrigger")

  const handleClick = (event: React.MouseEvent<Element>) => {
    onClick?.(event as unknown as React.MouseEvent<HTMLButtonElement>)
    if (!event.defaultPrevented) {
      setOpen(true)
    }
  }

  if (asChild && React.isValidElement<SlottableButtonProps>(children)) {
    const child = children

    return React.cloneElement(child, {
      ...(props as unknown as SlottableButtonProps),
      "data-slot": "dialog-trigger",
      onClick: (event: React.MouseEvent<Element>) => {
        child.props.onClick?.(event)
        handleClick(event)
      },
    })
  }

  return (
    <button data-slot="dialog-trigger" type="button" onClick={handleClick} {...props}>
      {children}
    </button>
  )
}

function DialogPortal({ children }: { children: React.ReactNode }) {
  const { open } = useDialogContext("DialogPortal")

  if (!open || typeof document === "undefined") {
    return null
  }

  return createPortal(children, document.body)
}

const DialogOverlay = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div
      ref={ref}
      data-slot="dialog-overlay"
      data-state="open"
      className={cn(
        "data-[state=open]:animate-in data-[state=open]:fade-in-0 fixed inset-0 z-50 bg-black/50",
        className
      )}
      {...props}
    />
  )
)
DialogOverlay.displayName = "DialogOverlay"

const DialogContent = React.forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement> & {
    showCloseButton?: boolean
    onInteractOutside?: (event: DialogInteractOutsideEvent) => void
  }
>(({ className, children, showCloseButton = true, onInteractOutside, ...props }, ref) => {
  const { open, setOpen } = useDialogContext("DialogContent")
  const contentRef = React.useRef<HTMLDivElement | null>(null)

  React.useEffect(() => {
    if (!open) {
      return undefined
    }

    const originalOverflow = document.body.style.overflow
    document.body.style.overflow = "hidden"

    return () => {
      document.body.style.overflow = originalOverflow
    }
  }, [open])

  React.useEffect(() => {
    if (!open) {
      return undefined
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault()
        setOpen(false)
      }
    }

    document.addEventListener("keydown", handleKeyDown)
    return () => document.removeEventListener("keydown", handleKeyDown)
  }, [open, setOpen])

  React.useEffect(() => {
    if (open) {
      contentRef.current?.focus()
    }
  }, [open])

  if (!open) {
    return null
  }

  const handleOverlayMouseDown = (event: React.MouseEvent<HTMLDivElement>) => {
    let defaultPrevented = false

    const interactOutsideEvent: DialogInteractOutsideEvent = {
      get defaultPrevented() {
        return defaultPrevented
      },
      originalEvent: event.nativeEvent,
      preventDefault: () => {
        defaultPrevented = true
      },
    }

    onInteractOutside?.(interactOutsideEvent)

    if (!defaultPrevented) {
      setOpen(false)
    }
  }

  return (
    <DialogPortal>
      <DialogOverlay onMouseDown={handleOverlayMouseDown} />
      <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
        <div
          ref={composeRefs(contentRef, ref)}
          data-slot="dialog-content"
          data-state="open"
          aria-modal="true"
          role="dialog"
          tabIndex={-1}
          className={cn(
            "bg-background data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95 fixed top-[50%] left-[50%] z-50 grid w-full max-w-[calc(100%-2rem)] translate-x-[-50%] translate-y-[-50%] gap-4 rounded-lg border p-6 shadow-lg outline-none sm:max-w-lg",
            className
          )}
          {...props}
        >
          {children}
          {showCloseButton && (
            <button
              data-slot="dialog-close"
              type="button"
              onClick={() => setOpen(false)}
              className="ring-offset-background focus:ring-ring data-[state=open]:bg-accent data-[state=open]:text-muted-foreground absolute top-4 right-4 rounded-xs opacity-70 transition-opacity hover:opacity-100 focus:ring-2 focus:ring-offset-2 focus:outline-hidden disabled:pointer-events-none [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4"
            >
              <XIcon />
              <span className="sr-only">Close</span>
            </button>
          )}
        </div>
      </div>
    </DialogPortal>
  )
})
DialogContent.displayName = "DialogContent"

function DialogClose({
  children,
  asChild = false,
  onClick,
  ...props
}: React.ButtonHTMLAttributes<HTMLButtonElement> & {
  asChild?: boolean
  children?: React.ReactNode
}) {
  const { setOpen } = useDialogContext("DialogClose")

  const handleClick = (event: React.MouseEvent<Element>) => {
    onClick?.(event as unknown as React.MouseEvent<HTMLButtonElement>)
    if (!event.defaultPrevented) {
      setOpen(false)
    }
  }

  if (asChild && React.isValidElement<SlottableButtonProps>(children)) {
    const child = children

    return React.cloneElement(child, {
      ...(props as unknown as SlottableButtonProps),
      "data-slot": "dialog-close",
      onClick: (event: React.MouseEvent<Element>) => {
        child.props.onClick?.(event)
        handleClick(event)
      },
    })
  }

  return (
    <button data-slot="dialog-close" type="button" onClick={handleClick} {...props}>
      {children}
    </button>
  )
}

function DialogHeader({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="dialog-header"
      className={cn("flex flex-col gap-2 text-center sm:text-left", className)}
      {...props}
    />
  )
}

function DialogFooter({
  className,
  showCloseButton = false,
  children,
  ...props
}: React.ComponentProps<"div"> & {
  showCloseButton?: boolean
}) {
  return (
    <div
      data-slot="dialog-footer"
      className={cn("flex flex-col-reverse gap-2 sm:flex-row sm:justify-end", className)}
      {...props}
    >
      {children}
      {showCloseButton && (
        <DialogClose asChild>
          <Button variant="outline">Close</Button>
        </DialogClose>
      )}
    </div>
  )
}

const DialogTitle = React.forwardRef<HTMLHeadingElement, React.ComponentPropsWithoutRef<"h2">>(
  ({ className, ...props }, ref) => (
    <h2
      ref={ref}
      data-slot="dialog-title"
      className={cn("text-lg leading-none font-semibold", className)}
      {...props}
    />
  )
)
DialogTitle.displayName = "DialogTitle"

const DialogDescription = React.forwardRef<
  HTMLParagraphElement,
  React.ComponentPropsWithoutRef<"p">
>(({ className, ...props }, ref) => (
  <p
    ref={ref}
    data-slot="dialog-description"
    className={cn("text-muted-foreground text-sm", className)}
    {...props}
  />
))
DialogDescription.displayName = "DialogDescription"

export {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogOverlay,
  DialogPortal,
  DialogTitle,
  DialogTrigger,
}
