import * as React from "react"

import { cn } from "@/lib/utils"

type SwitchProps = {
  checked?: boolean
  defaultChecked?: boolean
  disabled?: boolean
  onCheckedChange?: (checked: boolean) => void
} & Omit<React.ButtonHTMLAttributes<HTMLButtonElement>, "onChange">

function Switch({
  checked,
  defaultChecked = false,
  disabled = false,
  onCheckedChange,
  className,
  ...props
}: SwitchProps) {
  const [internalChecked, setInternalChecked] = React.useState(defaultChecked)
  const isControlled = checked !== undefined
  const resolvedChecked = isControlled ? checked : internalChecked

  const toggle = React.useCallback(() => {
    if (disabled) return
    const next = !resolvedChecked
    if (!isControlled) {
      setInternalChecked(next)
    }
    onCheckedChange?.(next)
  }, [disabled, isControlled, onCheckedChange, resolvedChecked])

  return (
    <button
      type="button"
      role="switch"
      aria-checked={resolvedChecked}
      data-state={resolvedChecked ? "checked" : "unchecked"}
      disabled={disabled}
      onClick={toggle}
      className={cn(
        "focus-visible:border-ring focus-visible:ring-ring/50 data-[state=checked]:bg-primary data-[state=unchecked]:bg-input inline-flex h-5 w-9 shrink-0 items-center rounded-full border border-transparent shadow-xs transition-colors outline-none focus-visible:ring-[3px] disabled:cursor-not-allowed disabled:opacity-50",
        className
      )}
      {...props}
    >
      <span
        data-slot="switch-thumb"
        className={cn(
          "bg-background pointer-events-none block size-4 rounded-full ring-0 transition-transform",
          resolvedChecked ? "translate-x-4" : "translate-x-0"
        )}
      />
    </button>
  )
}

export { Switch }
