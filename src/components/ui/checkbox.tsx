import * as React from "react"
import { Check } from "lucide-react"

import { cn } from "@/lib/utils"

type CheckboxProps = {
  checked?: boolean
  defaultChecked?: boolean
  disabled?: boolean
  onCheckedChange?: (checked: boolean) => void
} & Omit<React.ButtonHTMLAttributes<HTMLButtonElement>, "onChange">

function Checkbox({
  checked,
  defaultChecked = false,
  disabled = false,
  onCheckedChange,
  className,
  ...props
}: CheckboxProps) {
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
      role="checkbox"
      aria-checked={resolvedChecked}
      data-state={resolvedChecked ? "checked" : "unchecked"}
      disabled={disabled}
      onClick={toggle}
      className={cn(
        "border-input dark:bg-input/30 focus-visible:border-ring focus-visible:ring-ring/50 data-[state=checked]:bg-primary data-[state=checked]:text-primary-foreground inline-flex size-4 shrink-0 items-center justify-center rounded-[4px] border bg-transparent shadow-xs transition-all outline-none focus-visible:ring-[3px] disabled:cursor-not-allowed disabled:opacity-50",
        className
      )}
      {...props}
    >
      <Check className={cn("size-3", resolvedChecked ? "opacity-100" : "opacity-0")} />
    </button>
  )
}

export { Checkbox }
