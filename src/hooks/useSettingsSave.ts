import { useCallback } from "react"
import { useTranslation } from "react-i18next"

import { useConfig, type AppConfig } from "@/contexts/ConfigContext"
import { useToast } from "@/hooks/use-toast"
import { toErrorMessage } from "@/lib/utils"

interface UseSettingsSaveOptions {
  successTitle?: string
  successDescription?: string
  errorTitle?: string
}

export function useSettingsSave() {
  const { saveConfig } = useConfig()
  const { toast } = useToast()
  const { t } = useTranslation()

  const saveSettings = useCallback(
    async (updates: Partial<AppConfig>, options?: UseSettingsSaveOptions) => {
      try {
        await saveConfig(updates)
        if (options?.successTitle) {
          toast({
            title: options.successTitle,
            description: options.successDescription,
          })
        }
        return true
      } catch (error) {
        console.error("Failed to save settings:", error)
        toast({
          title:
            options?.errorTitle ??
            t("settings.saveFailed", { defaultValue: "Failed to save settings" }),
          description: toErrorMessage(error),
          variant: "destructive",
        })
        return false
      }
    },
    [saveConfig, toast, t]
  )

  return { saveSettings }
}
