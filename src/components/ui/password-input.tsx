import * as React from "react"
import { Eye, EyeOff } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { cn } from "@/lib/utils"

interface PasswordInputProps extends Omit<React.ComponentProps<"input">, "type"> {
  containerClassName?: string
  // Hide the toggle when the field holds a placeholder mask rather than a real secret.
  revealable?: boolean
}

const PasswordInput = React.forwardRef<HTMLInputElement, PasswordInputProps>(
  ({ className, containerClassName, revealable = true, disabled, ...props }, ref) => {
    const { t } = useTranslation()
    const [visible, setVisible] = React.useState(false)
    const shown = revealable && visible
    const toggleLabel = t(shown ? "secretStorage.hidePassword" : "secretStorage.showPassword")

    return (
      <div className={cn("relative", containerClassName)}>
        <Input
          ref={ref}
          type={shown ? "text" : "password"}
          disabled={disabled}
          className={cn(revealable && "pr-9", className)}
          {...props}
        />
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
