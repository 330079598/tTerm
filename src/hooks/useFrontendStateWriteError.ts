import { useEffect } from "react"
import { useTranslation } from "react-i18next"

import { toast } from "@/hooks/use-toast"
import {
  onFrontendStateWriteError,
  takePendingFrontendStateWriteError,
} from "@/lib/frontendStateCache"

/** Shows a write that failed after the value was already used from the cache. */
export function useFrontendStateWriteError(): void {
  const { t } = useTranslation()

  useEffect(() => {
    const show = (message: string) => {
      toast({
        title: t("frontendState.writeFailed"),
        description: message,
        variant: "destructive",
      })
    }
    const pending = takePendingFrontendStateWriteError()
    if (pending) show(pending)
    return onFrontendStateWriteError(show)
  }, [t])
}
