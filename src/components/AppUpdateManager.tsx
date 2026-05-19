import { useEffect, useState } from "react"
import { Download, PackageCheck, RotateCcw } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { UpdateReleaseNotes } from "@/components/UpdateReleaseNotes"
import { useConfig } from "@/contexts/ConfigContext"
import {
  downloadAndInstallAppUpdate,
  installDownloadedAppUpdate,
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
  const [dismissedStatusKey, setDismissedStatusKey] = useState<string | null>(null)

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

  const statusKey = updateState ? `${updateState.status}:${updateState.latestVersion ?? ""}` : null
  const shouldShowUpdateDialog =
    updateState &&
    statusKey !== dismissedStatusKey &&
    ((updateState.status === "available" && !config.auto_download_updates) ||
      updateState.status === "downloaded" ||
      updateState.status === "ready")
  const dialogState = shouldShowUpdateDialog ? updateState : null
  const closeDialog = () => {
    setDismissedStatusKey(statusKey)
  }

  const dialogTitle =
    dialogState?.status === "ready"
      ? t("updates.readyTitle", { defaultValue: "Update ready" })
      : dialogState?.status === "downloaded"
        ? t("updates.downloadedTitle", { defaultValue: "Update downloaded" })
        : t("updates.availableTitle", { defaultValue: "Update available" })
  const dialogDescription =
    dialogState?.status === "ready"
      ? t("updates.readyDesc", {
          defaultValue: "Restart tTerm when you are ready to finish installing {{version}}.",
          version: dialogState.latestVersion,
        })
      : dialogState?.status === "downloaded"
        ? t("updates.downloadedDesc", {
            defaultValue: "Version {{version}} has been downloaded. Install it now?",
            version: dialogState.latestVersion,
          })
        : t("updates.availableDesc", {
            defaultValue: "Version {{version}} is ready to download.",
            version: dialogState?.latestVersion,
          })
  const releaseNotes = dialogState?.notes?.trim()

  return (
    <Dialog open={Boolean(dialogState)} onOpenChange={(open) => !open && closeDialog()}>
      <DialogContent className="sm:max-w-[620px]">
        <DialogHeader>
          <DialogTitle>{dialogTitle}</DialogTitle>
          <DialogDescription>{dialogDescription}</DialogDescription>
        </DialogHeader>

        {releaseNotes && (
          <div className="bg-muted/50 space-y-2 rounded-md border p-3">
            <div className="text-xs font-medium">
              {t("updates.releaseNotes", { defaultValue: "What's new" })}
            </div>
            <div className="text-muted-foreground max-h-72 overflow-auto text-xs leading-5 break-words">
              <UpdateReleaseNotes notes={releaseNotes} />
            </div>
          </div>
        )}

        <DialogFooter>
          <Button type="button" variant="outline" onClick={closeDialog}>
            {t("common.close", { defaultValue: "Close" })}
          </Button>
          {dialogState?.status === "available" && (
            <Button
              type="button"
              onClick={() => {
                closeDialog()
                void downloadAndInstallAppUpdate(config.update_channel)
              }}
            >
              <Download size={14} />
              {t("updates.downloadInstall", { defaultValue: "Download and install" })}
            </Button>
          )}
          {dialogState?.status === "downloaded" && (
            <Button
              type="button"
              onClick={() => {
                closeDialog()
                void installDownloadedAppUpdate(config.update_channel)
              }}
            >
              <PackageCheck size={14} />
              {t("updates.install", { defaultValue: "Install" })}
            </Button>
          )}
          {dialogState?.status === "ready" && (
            <Button type="button" onClick={() => void relaunchApp()}>
              <RotateCcw size={14} />
              {t("updates.restart", { defaultValue: "Restart" })}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
