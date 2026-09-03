import { AlertCircle, CheckCircle2 } from "lucide-react"

import {
  Toast,
  ToastClose,
  ToastDescription,
  ToastProvider,
  ToastTitle,
  ToastViewport,
} from "@/components/ui/toast"
import { useToast } from "@/hooks/use-toast"

export function Toaster() {
  const { toasts, dismiss } = useToast()

  return (
    <ToastProvider>
      {toasts.map(function ({ id, title, description, action, onOpenChange, ...props }) {
        return (
          <Toast
            key={id}
            {...props}
            open
            onOpenChange={(open) => {
              onOpenChange?.(open)
              if (!open) {
                dismiss(id)
              }
            }}
          >
            <div className="flex items-start gap-3">
              {props.variant === "success" && (
                <CheckCircle2 className="mt-0.5 size-4 shrink-0" />
              )}
              {props.variant === "destructive" && (
                <AlertCircle className="mt-0.5 size-4 shrink-0" />
              )}
              <div className="grid gap-1">
                {title && <ToastTitle>{title}</ToastTitle>}
                {description && <ToastDescription>{description}</ToastDescription>}
              </div>
            </div>
            {action}
            <ToastClose onClick={() => dismiss(id)} />
          </Toast>
        )
      })}
      <ToastViewport />
    </ToastProvider>
  )
}
