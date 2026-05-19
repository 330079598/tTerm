import { useEffect, useRef, useState } from "react"
import { RotateCcw } from "lucide-react"
import { useTranslation } from "react-i18next"

import { ToastAction } from "@/components/ui/toast"
import { useConfig } from "@/contexts/ConfigContext"
import { toast } from "@/hooks/use-toast"
import {
  relaunchApp,
  startBackgroundUpdateChecks,
  stopBackgroundUpdateChecks,
  subscribeToUpdater,
  type UpdateState,
} from "@/lib/updater"

export function AppUpdateManager() {
  const { t } = useTranslation()
  const { config, isLoaded, saveConfig } = useConfig()
  const [updateState, setUpdateState] = useState<UpdateState | null>(null)
  const lastNotifiedStatusRef = useRef<string | null>(null)

  useEffect(() => {
    const unsubscribe = subscribeToUpdater(setUpdateState)
    return () => {
      unsubscribe()
    }
  }, [])

  useEffect(() => {
    if (!isLoaded) {
      return
    }

    startBackgroundUpdateChecks(
      config.update_channel,
      config.auto_download_updates,
      config.update_check_frequency,
      config.last_update_check_at,
      (checkedAt) => {
        saveConfig({ last_update_check_at: checkedAt }).catch((error) => {
          console.error("Failed to save update check timestamp:", error)
        })
      }
    )
    return stopBackgroundUpdateChecks
  }, [
    config.auto_download_updates,
    config.last_update_check_at,
    config.update_channel,
    config.update_check_frequency,
    isLoaded,
    saveConfig,
  ])

  useEffect(() => {
    if (!updateState) {
      return
    }

    const statusKey = `${updateState.status}:${updateState.latestVersion ?? ""}`
    if (lastNotifiedStatusRef.current === statusKey) {
      return
    }

    if (updateState.status === "available" && !config.auto_download_updates) {
      lastNotifiedStatusRef.current = statusKey
      toast({
        title: t("updates.availableTitle", { defaultValue: "Update available" }),
        description: t("updates.availableDesc", {
          defaultValue: "Version {{version}} is ready to download.",
          version: updateState.latestVersion,
        }),
      })
    }

    if (updateState.status === "ready") {
      lastNotifiedStatusRef.current = statusKey
      toast({
        title: t("updates.readyTitle", { defaultValue: "Update ready" }),
        description: t("updates.readyDesc", {
          defaultValue: "Restart tTerm when you are ready to finish installing {{version}}.",
          version: updateState.latestVersion,
        }),
        duration: 20_000,
        action: (
          <ToastAction
            altText={t("updates.restart", { defaultValue: "Restart" })}
            onClick={relaunchApp}
          >
            <RotateCcw size={14} />
            {t("updates.restart", { defaultValue: "Restart" })}
          </ToastAction>
        ),
      })
    }
  }, [config.auto_download_updates, t, updateState])

  return null
}
