import * as React from "react"
import { ArrowBigUpDash, Eye, EyeOff } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { useCapsLock } from "@/hooks/useCapsLock"
import { cn } from "@/lib/utils"

interface PasswordInputProps extends Omit<React.ComponentProps<"input">, "type"> {
  containerClassName?: string
  // Hide the toggle when the field holds a placeholder mask rather than a real secret.
  revealable?: boolean
}

const PasswordInput = React.forwardRef<HTMLInputElement, PasswordInputProps>(
  (
    { className, containerClassName, revealable = true, disabled, onFocus, onBlur, ...props },
    ref
  ) => {
    const { t } = useTranslation()
    const [visible, setVisible] = React.useState(false)
    const [focused, setFocused] = React.useState(false)
    const capsLockOn = useCapsLock()
    const shown = revealable && visible
    // A revealed password shows the case of what is typed, so it needs no warning.
    const warnCapsLock = focused && capsLockOn && !shown && !disabled
    const toggleLabel = t(shown ? "secretStorage.hidePassword" : "secretStorage.showPassword")
    const capsLockLabel = t("common.capsLockOn")

    return (
      <div className={cn("relative", containerClassName)}>
        <Input
          ref={ref}
          type={shown ? "text" : "password"}
          disabled={disabled}
          className={cn(
            revealable && "pr-9",
            warnCapsLock && (revealable ? "pr-16" : "pr-9"),
            className
          )}
          onFocus={(event) => {
            setFocused(true)
            onFocus?.(event)
          }}
          onBlur={(event) => {
            setFocused(false)
            onBlur?.(event)
          }}
          {...props}
        />
        {warnCapsLock && (
          <span
            role="status"
            aria-label={capsLockLabel}
            title={capsLockLabel}
            // Pressing the icon would otherwise blur the field and hide it.
            onMouseDown={(event) => event.preventDefault()}
            className={cn(
              "text-warning absolute top-1/2 -translate-y-1/2",
              revealable ? "right-9" : "right-2.5"
            )}
          >
            <ArrowBigUpDash size={16} aria-hidden="true" />
          </span>
        )}
        {revealable && (
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            disabled={disabled}
            onClick={() => setVisible((current) => !current)}
            className="text-muted-foreground hover:text-foreground absolute top-1/2 right-1 -translate-y-1/2"
            aria-label={toggleLabel}
            title={toggleLabel}
          >
            {shown ? <EyeOff size={16} /> : <Eye size={16} />}
          </Button>
        )}
      </div>
    )
  }
)
PasswordInput.displayName = "PasswordInput"

export { PasswordInput }
