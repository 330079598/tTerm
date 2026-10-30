import * as React from "react"

import { cn } from "@/lib/utils"

const Input = React.forwardRef<HTMLInputElement, React.ComponentProps<"input">>(
  ({ className, type, ...props }, ref) => {
    return (
      <input
        ref={ref}
        type={type}
        data-slot="input"
        // Connection and terminal values must be preserved exactly as entered.
        // Individual callers can still opt into a browser input aid via props.
        autoCapitalize="none"
        autoCorrect="off"
        autoComplete="off"
        spellCheck={false}
        className={cn(
          "border-input selection:bg-primary selection:text-primary-foreground file:text-foreground placeholder:text-muted-foreground dark:bg-input/30 h-9 w-full min-w-0 rounded-md border bg-transparent px-3 py-1 text-base shadow-xs transition-[color,box-shadow] outline-none file:inline-flex file:h-7 file:border-0 file:bg-transparent file:text-sm file:font-medium disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 md:text-sm",
          // A field shows focus with its border alone. A halo around the border
          // drew a second, heavy frame, as the broadcast panel found first.
          "focus-visible:border-ring",
          "aria-invalid:border-destructive",
          className
        )}
        {...props}
      />
    )
  }
)
Input.displayName = "Input"

export { Input }
