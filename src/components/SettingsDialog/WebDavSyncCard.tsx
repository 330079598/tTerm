import React, { useCallback, useEffect, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { AlertTriangle, CheckCircle2, Info, Loader2, RefreshCw, RotateCcw } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { Checkbox } from "@/components/ui/checkbox"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import { useToast } from "@/hooks/use-toast"
import {
  runSync,
  SYNC_PULL_INTERVALS_MINS,
  SYNC_PUSH_INTERVALS_SECS,
  SYNC_SETTINGS_EVENT,
  SYNC_STATUS_EVENT,
  type SyncSelection,
  type SyncSettings,
} from "@/lib/sync"
import { toErrorMessage } from "@/lib/utils"

interface SyncStatus {
  settings: SyncSettings
  lastSyncedAt: number | null
  lastError: string | null
}

type BusyAction = "save" | "sync" | "reset"

export const WebDavSyncCard: React.FC = () => {
  const { t } = useTranslation()
  const { toast } = useToast()
  const [status, setStatus] = useState<SyncStatus | null>(null)
  const [busyAction, setBusyAction] = useState<BusyAction | null>(null)
  const busy = busyAction !== null

  const refresh = useCallback(() => {
    invoke<SyncStatus>("get_sync_status")
      .then(setStatus)
      .catch((error) => console.error("Failed to load sync status:", error))
  }, [])

  useEffect(() => {
    refresh()
    window.addEventListener(SYNC_STATUS_EVENT, refresh)
    return () => window.removeEventListener(SYNC_STATUS_EVENT, refresh)
  }, [refresh])

  if (!status) return null
  const { settings } = status

  const syncNow = async () => {
    setBusyAction("sync")
    try {
      const outcome = await runSync(true)
      if (outcome.state === "locked") {
        toast({ title: t("dataMigration.sync.locked"), variant: "destructive" })
      } else if (outcome.state === "busy") {
        toast({ title: t("dataMigration.sync.busy") })
      } else if (outcome.state === "synced") {
        toast({
          title: t("dataMigration.sync.done"),
          description:
            outcome.changed.length > 0
              ? t("dataMigration.sync.changed", {
                  categories: outcome.changed
                    .map((category) => t(`dataMigration.sync.categoryNames.${category}`))
                    .join(t("dataMigration.sync.listSeparator")),
                })
              : t("dataMigration.sync.upToDate"),
        })
      }
    } catch (error) {
      toast({
        title: t("dataMigration.sync.failed"),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    } finally {
      setBusyAction(null)
    }
  }

  const save = async (next: SyncSettings) => {
    setBusyAction("save")
    try {
      setStatus(await invoke<SyncStatus>("save_sync_settings", { settings: next }))
      window.dispatchEvent(new Event(SYNC_SETTINGS_EVENT))
    } catch (error) {
      toast({
        title: t("dataMigration.sync.saveFailed"),
        description: toErrorMessage(error),
        variant: "destructive",
      })
      return
    } finally {
      setBusyAction(null)
    }
    if (next.enabled && !settings.enabled) await syncNow()
  }

  const updateSelection = (key: keyof SyncSelection, checked: boolean) => {
    const selection = { ...settings.selection, [key]: checked }
    if (key === "secrets" && checked) selection.profiles = true
    if (key === "profiles" && !checked) selection.secrets = false
    void save({ ...settings, selection })
  }

  const handleReset = async () => {
    if (!window.confirm(t("dataMigration.sync.resetConfirm"))) return
    setBusyAction("reset")
    try {
      setStatus(await invoke<SyncStatus>("reset_sync"))
      toast({ title: t("dataMigration.sync.resetDone") })
    } catch (error) {
      toast({
        title: t("dataMigration.sync.resetFailed"),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    } finally {
      setBusyAction(null)
    }
  }

  const categories: Array<{ key: keyof SyncSelection; label: string }> = [
    { key: "profiles", label: t("dataMigration.sync.categories.profiles") },
    { key: "secrets", label: t("dataMigration.sync.categories.secrets") },
    { key: "commands", label: t("dataMigration.sync.categories.commands") },
    { key: "knownHosts", label: t("dataMigration.sync.categories.knownHosts") },
    { key: "settings", label: t("dataMigration.sync.categories.settings") },
    { key: "themes", label: t("dataMigration.sync.categories.themes") },
  ]

  return (
    <Card>
      <CardContent className="space-y-4 p-4">
        <div className="flex items-start justify-between gap-3">
          <div>
            <div className="flex items-center gap-2 text-sm font-medium">
              <RefreshCw size={16} />
              {t("dataMigration.sync.title")}
            </div>
            <p className="text-muted-foreground mt-1 text-xs">
              {t("dataMigration.sync.description")}
            </p>
          </div>
          <Switch
            checked={settings.enabled}
            disabled={busy}
            aria-label={t("dataMigration.sync.title")}
            onCheckedChange={(enabled) => void save({ ...settings, enabled })}
          />
        </div>

        {settings.enabled && (
          <>
            <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
              {categories.map((item) => (
                <label
                  key={item.key}
                  className="border-border flex min-h-10 cursor-pointer items-center gap-3 rounded-md border px-3 py-2 text-sm"
                >
                  <Checkbox
                    checked={settings.selection[item.key]}
                    disabled={busy}
                    onCheckedChange={(checked) => updateSelection(item.key, checked)}
                  />
                  <span>{item.label}</span>
                </label>
              ))}
            </div>

            <div className="grid gap-3 sm:grid-cols-2">
              <div>
                <Label htmlFor="sync-push-interval">{t("dataMigration.sync.pushInterval")}</Label>
                <Select
                  id="sync-push-interval"
                  className="mt-1.5"
                  value={settings.pushIntervalSecs}
                  disabled={busy}
                  onChange={(event) =>
                    void save({ ...settings, pushIntervalSecs: Number(event.target.value) })
                  }
                >
                  {SYNC_PUSH_INTERVALS_SECS.map((seconds) => (
                    <option key={seconds} value={seconds}>
                      {seconds < 60
                        ? t("dataMigration.sync.seconds", { count: seconds })
                        : t("dataMigration.sync.minutes", { count: seconds / 60 })}
                    </option>
                  ))}
                </Select>
              </div>
              <div>
                <Label htmlFor="sync-pull-interval">{t("dataMigration.sync.pullInterval")}</Label>
                <Select
                  id="sync-pull-interval"
                  className="mt-1.5"
                  value={settings.pullIntervalMins}
                  disabled={busy}
                  onChange={(event) =>
                    void save({ ...settings, pullIntervalMins: Number(event.target.value) })
                  }
                >
                  {SYNC_PULL_INTERVALS_MINS.map((minutes) => (
                    <option key={minutes} value={minutes}>
                      {t("dataMigration.sync.minutes", { count: minutes })}
                    </option>
                  ))}
                </Select>
              </div>
            </div>
            <p className="text-muted-foreground -mt-2 text-xs">
              {t("dataMigration.sync.intervalHint")}
            </p>

            <Alert>
              <Info size={16} className="absolute top-3.5 left-4" />
              <AlertDescription className="space-y-1 pl-6">
                <p>{t("dataMigration.sync.firstSyncNote")}</p>
                <p>{t("dataMigration.sync.localOnlyNote")}</p>
                <p>{t("dataMigration.sync.passwordNote")}</p>
              </AlertDescription>
            </Alert>

            {status.lastError ? (
              <Alert className="border-destructive/40 bg-destructive/10">
                <AlertTriangle size={16} className="absolute top-3.5 left-4" />
                <div className="pl-6">
                  <AlertTitle>{t("dataMigration.sync.lastFailed")}</AlertTitle>
                  <AlertDescription className="break-all">{status.lastError}</AlertDescription>
                </div>
              </Alert>
            ) : (
              status.lastSyncedAt && (
                <p className="text-muted-foreground flex items-center gap-1.5 text-xs">
                  <CheckCircle2 size={14} className="text-emerald-500" />
                  {t("dataMigration.sync.lastSyncedAt", {
                    time: new Date(status.lastSyncedAt).toLocaleString(),
                  })}
                </p>
              )
            )}

            <div className="flex flex-wrap gap-2">
              <Button variant="outline" disabled={busy} onClick={() => void syncNow()}>
                {busyAction === "sync" ? (
                  <Loader2 className="animate-spin" size={16} />
                ) : (
                  <RefreshCw size={16} />
                )}
                {busyAction === "sync"
                  ? t("dataMigration.sync.syncing")
                  : t("dataMigration.sync.now")}
              </Button>
              <Button
                variant="ghost"
                className="ml-auto"
                disabled={busy}
                onClick={() => void handleReset()}
              >
                <RotateCcw size={16} />
                {t("dataMigration.sync.reset")}
              </Button>
            </div>
          </>
        )}
      </CardContent>
    </Card>
  )
}
