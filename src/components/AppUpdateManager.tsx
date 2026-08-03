import { lazy, Suspense, useEffect, useRef, useState } from "react"
import { Download, PackageCheck } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { ToastAction } from "@/components/ui/toast"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { useConfig } from "@/contexts/ConfigContext"
import { useAppActivity } from "@/contexts/AppActivityContext"
import { toast } from "@/hooks/use-toast"
import {
  downloadAndInstallAppUpdate,
  installDownloadedAppUpdate,
  retryAppUpdate,
  startBackgroundUpdateChecks,
  stopBackgroundUpdateChecks,
  subscribeToUpdater,
  type UpdateState,
} from "@/lib/updater"

const UpdateReleaseNotes = lazy(() =>
  import("@/components/UpdateReleaseNotes").then((module) => ({
    default: module.UpdateReleaseNotes,
  }))
)

export function AppUpdateManager() {
  const { t } = useTranslation()
  const { requestAppRestart } = useAppActivity()
  const { config, isLoaded, saveConfig } = useConfig()
  const [updateState, setUpdateState] = useState<UpdateState | null>(null)
  const [dismissedStatusKey, setDismissedStatusKey] = useState<string | null>(null)
  const notificationRef = useRef<ReturnType<typeof toast> | null>(null)
  const dismissedNotificationStatusRef = useRef<UpdateState["status"] | null>(null)

  useEffect(() => {
    const unsubscribe = subscribeToUpdater(setUpdateState)
    return () => {
      unsubscribe()
    }
  }, [])

  useEffect(() => {
    if (!updateState) {
      return
    }

    const showNotification = (options: Parameters<typeof toast>[0]) => {
      if (dismissedNotificationStatusRef.current === updateState.status) {
        return
      }

      const notificationOptions = {
        ...options,
        onOpenChange: (open: boolean) => {
          options.onOpenChange?.(open)
          if (!open) {
            notificationRef.current = null
            dismissedNotificationStatusRef.current = updateState.status
          }
        },
      }
      if (notificationRef.current) {
        notificationRef.current.update(notificationOptions)
      } else {
        notificationRef.current = toast(notificationOptions)
      }
    }

    if (dismissedNotificationStatusRef.current !== updateState.status) {
      dismissedNotificationStatusRef.current = null
    }

    if (updateState.status === "downloading") {
      const progress = updateState.totalBytes
        ? Math.min(100, Math.round((updateState.downloadedBytes / updateState.totalBytes) * 100))
        : null
      showNotification({
        title: t("updates.downloadingTitle", { defaultValue: "Downloading update" }),
        description:
          progress === null
            ? t("updates.downloadingDesc", {
                defaultValue: "Downloading version {{version}} in the background.",
                version: updateState.latestVersion,
              })
            : t("updates.downloadingProgress", {
                defaultValue: "Version {{version}}: {{progress}}%",
                version: updateState.latestVersion,
                progress,
              }),
        duration: Number.POSITIVE_INFINITY,
        variant: "default",
        action: undefined,
      })
      return
    }

    if (updateState.status === "installing") {
      showNotification({
        title: t("updates.installingTitle", { defaultValue: "Installing update" }),
        description: t("updates.installingDesc", {
          defaultValue: "Installing version {{version}}. You can continue working.",
          version: updateState.latestVersion,
        }),
        duration: Number.POSITIVE_INFINITY,
        variant: "default",
        action: undefined,
      })
      return
    }

    if (updateState.status === "ready") {
      showNotification({
        title: t("updates.readyTitle", { defaultValue: "Update ready" }),
        description: t("updates.readyDesc", {
          defaultValue: "Version {{version}} is installed. Restart tTerm to use it.",
          version: updateState.latestVersion,
        }),
        duration: Number.POSITIVE_INFINITY,
        variant: "success",
        action: (
          <ToastAction
            altText={t("updates.restart", { defaultValue: "Restart" })}
            onClick={() => void requestAppRestart()}
          >
            {t("updates.restart", { defaultValue: "Restart" })}
          </ToastAction>
        ),
      })
      return
    }

    if (updateState.status === "error") {
      const errorDescription =
        updateState.errorCode === "downloaded-update-unavailable"
          ? t("updates.downloadedUnavailable", {
              defaultValue:
                "The downloaded update is no longer available. Check for updates again.",
            })
          : updateState.error
      showNotification({
        title: t("updates.failedTitle", { defaultValue: "Update failed" }),
        description: errorDescription,
        duration: Number.POSITIVE_INFINITY,
        variant: "destructive",
        action: updateState.canRetry ? (
          <ToastAction
            altText={t("common.retry", { defaultValue: "Retry" })}
            onClick={() => void retryAppUpdate()}
          >
            {t("common.retry", { defaultValue: "Retry" })}
          </ToastAction>
        ) : undefined,
      })
      return
    }

    if (notificationRef.current) {
      notificationRef.current.dismiss()
      notificationRef.current = null
    }
  }, [requestAppRestart, t, updateState])

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
      updateState.status === "downloaded")
  const dialogState = shouldShowUpdateDialog ? updateState : null
  const closeDialog = () => {
    setDismissedStatusKey(statusKey)
  }

  const dialogTitle =
    dialogState?.status === "downloaded"
      ? t("updates.downloadedTitle", { defaultValue: "Update downloaded" })
      : t("updates.availableTitle", { defaultValue: "Update available" })
  const dialogDescription =
    dialogState?.status === "downloaded"
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
      <DialogContent
        className="sm:max-w-[620px]"
        overlayClassName="bg-background backdrop-blur-none"
        overlayDragRegion
        onInteractOutside={(event) => event.preventDefault()}
      >
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
              <Suspense fallback={<div className="whitespace-pre-wrap">{releaseNotes}</div>}>
                <UpdateReleaseNotes notes={releaseNotes} />
              </Suspense>
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
                void installDownloadedAppUpdate(dialogState.channel)
              }}
            >
              <PackageCheck size={14} />
              {t("updates.install", { defaultValue: "Install" })}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
