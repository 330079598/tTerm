import React, { useEffect, useMemo, useState } from "react"
import ReactMarkdown from "react-markdown"
import remarkEmoji from "remark-emoji"
import remarkGfm from "remark-gfm"
import {
  CalendarClock,
  CheckCircle2,
  Download,
  RefreshCw,
  RotateCcw,
  Rocket,
  Send,
} from "lucide-react"
import { useTranslation } from "react-i18next"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Select } from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import {
  checkForAppUpdate,
  downloadAndInstallAppUpdate,
  relaunchApp,
  subscribeToUpdater,
  type UpdateChannel,
  type UpdateCheckFrequency,
  type UpdateState,
} from "@/lib/updater"

interface UpdateSettingsTabProps {
  autoDownloadUpdates: boolean
  handleAutoDownloadUpdatesChange: (checked: boolean) => Promise<void>
  handleUpdateCheckFrequencyChange: (frequency: UpdateCheckFrequency) => Promise<void>
  handleUpdateChannelChange: (channel: UpdateChannel) => Promise<void>
  handleUpdateCheckComplete: (checkedAt: number) => Promise<void>
  updateChannel: UpdateChannel
  updateCheckFrequency: UpdateCheckFrequency
  lastUpdateCheckAt: number | null
}

function isSafeMarkdownHref(href: string | undefined) {
  if (!href) return false
  const trimmed = href.trim().toLowerCase()
  return (
    trimmed.startsWith("https://") || trimmed.startsWith("http://") || trimmed.startsWith("mailto:")
  )
}

const releaseNotesEmojiFallbacks: Record<string, string> = {
  technologist: "\u{1F9D1}\u200D\u{1F4BB}",
}

function normalizereleaseNotesEmojiFallbacks(notes: string) {
  return notes.replace(/:([\w+-]+):/g, (match, shortcode: string) => {
    return releaseNotesEmojiFallbacks[shortcode] ?? match
  })
}

function formatTimestamp(timestamp: number | null) {
  return timestamp ? new Date(timestamp).toLocaleString() : null
}

function formatBytes(bytes: number) {
  if (!bytes) return "0 B"
  const units = ["B", "KB", "MB", "GB"]
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1)
  return `${(bytes / 1024 ** index).toFixed(index === 0 ? 0 : 1)} ${units[index]}`
}

export const UpdateSettingsTab: React.FC<UpdateSettingsTabProps> = ({
  autoDownloadUpdates,
  handleAutoDownloadUpdatesChange,
  handleUpdateCheckFrequencyChange,
  handleUpdateChannelChange,
  handleUpdateCheckComplete,
  updateChannel,
  updateCheckFrequency,
  lastUpdateCheckAt,
}) => {
  const { t } = useTranslation()
  const [state, setState] = useState<UpdateState | null>(null)

  useEffect(() => {
    const unsubscribe = subscribeToUpdater(setState)
    return () => {
      unsubscribe()
    }
  }, [])

  const progress = useMemo(() => {
    if (!state?.totalBytes) return 0
    return Math.min(100, Math.round((state.downloadedBytes / state.totalBytes) * 100))
  }, [state])

  const checking = state?.status === "checking"
  const downloading = state?.status === "downloading"
  const hasUpdate = state?.status === "available"
  const ready = state?.status === "ready"
  const releaseNotes = state?.notes?.trim()
  const lastUpdateCheckLabel = formatTimestamp(lastUpdateCheckAt)
  const renderedReleaseNotes = useMemo(
    () => (releaseNotes ? normalizereleaseNotesEmojiFallbacks(releaseNotes) : ""),
    [releaseNotes]
  )

  return (
    <ScrollArea className="h-full pr-4">
      <div className="space-y-3">
        <Card className="overflow-hidden border-transparent shadow-none">
          <CardContent className="p-4">
            <div className="mb-3 flex items-start justify-between gap-4">
              <div className="flex items-start gap-3">
                <Rocket size={16} className="mt-0.5 shrink-0" />
                <div>
                  <div className="text-sm font-medium">
                    {t("updates.channel", { defaultValue: "Update channel" })}
                  </div>
                  <div className="text-muted-foreground text-xs leading-5">
                    {t("updates.channelDesc", {
                      defaultValue:
                        "Stable follows normal releases. beta-dev follows development preview builds.",
                    })}
                  </div>
                </div>
              </div>
              <Badge variant="outline">{updateChannel}</Badge>
            </div>
            <div className="grid grid-cols-2 gap-2">
              <Button
                type="button"
                variant={updateChannel === "stable" ? "default" : "outline"}
                onClick={() => handleUpdateChannelChange("stable")}
              >
                {t("updates.stable", { defaultValue: "Stable" })}
              </Button>
              <Button
                type="button"
                variant={updateChannel === "beta-dev" ? "default" : "outline"}
                onClick={() => handleUpdateChannelChange("beta-dev")}
              >
                {t("updates.betaDev", { defaultValue: "Beta Dev" })}
              </Button>
            </div>
          </CardContent>
        </Card>

        <Card className="overflow-hidden border-transparent shadow-none">
          <CardContent className="flex items-center justify-between gap-4 p-4">
            <div className="flex items-start gap-3">
              <Download size={16} className="mt-0.5 shrink-0" />
              <div>
                <div className="text-sm font-medium">
                  {t("updates.autoDownload", { defaultValue: "Download updates in background" })}
                </div>
                <div className="text-muted-foreground text-xs leading-5">
                  {t("updates.autoDownloadDesc", {
                    defaultValue:
                      "When an automatic check finds an update, tTerm downloads it in the background without interrupting active sessions.",
                  })}
                </div>
              </div>
            </div>
            <Switch
              checked={autoDownloadUpdates}
              onCheckedChange={handleAutoDownloadUpdatesChange}
            />
          </CardContent>
        </Card>

        <Card className="overflow-hidden border-transparent shadow-none">
          <CardContent className="flex items-center justify-between gap-4 p-4">
            <div className="flex items-start gap-3">
              <CalendarClock size={16} className="mt-0.5 shrink-0" />
              <div>
                <div className="text-sm font-medium">
                  {t("updates.checkFrequency", { defaultValue: "Automatic check frequency" })}
                </div>
                <div className="text-muted-foreground text-xs leading-5">
                  {t("updates.checkFrequencyDesc", {
                    defaultValue:
                      "Choose the minimum time between automatic checks for the selected channel.",
                  })}
                  {lastUpdateCheckLabel && (
                    <div>
                      {t("updates.lastChecked", {
                        defaultValue: "Last checked: {{time}}",
                        time: lastUpdateCheckLabel,
                      })}
                    </div>
                  )}
                </div>
              </div>
            </div>
            <Select
              className="w-40"
              value={updateCheckFrequency}
              onChange={(event) =>
                handleUpdateCheckFrequencyChange(event.target.value as UpdateCheckFrequency)
              }
            >
              <option value="daily">
                {t("updates.frequencyDaily", { defaultValue: "Daily" })}
              </option>
              <option value="every-3-days">
                {t("updates.frequencyEvery3Days", { defaultValue: "Every 3 days" })}
              </option>
              <option value="weekly">
                {t("updates.frequencyWeekly", { defaultValue: "Weekly" })}
              </option>
              <option value="never">
                {t("updates.frequencyNever", { defaultValue: "Never" })}
              </option>
            </Select>
          </CardContent>
        </Card>

        <Card className="overflow-hidden border-transparent shadow-none">
          <CardContent className="space-y-4 p-4">
            <div className="flex items-start justify-between gap-4">
              <div className="flex items-start gap-3">
                <RefreshCw size={16} className="mt-0.5 shrink-0" />
                <div>
                  <div className="text-sm font-medium">
                    {t("updates.manualCheck", { defaultValue: "Manual update check" })}
                  </div>
                  <div className="text-muted-foreground text-xs leading-5">
                    {state?.status === "not-available"
                      ? t("updates.noUpdate", {
                          defaultValue: "You are already on the latest version.",
                        })
                      : state?.status === "error"
                        ? state.error
                        : ready
                          ? t("updates.readyDesc", {
                              defaultValue:
                                "Restart tTerm when you are ready to finish installing {{version}}.",
                              version: state.latestVersion,
                            })
                          : hasUpdate
                            ? t("updates.availableDesc", {
                                defaultValue: "Version {{version}} is ready to download.",
                                version: state.latestVersion,
                              })
                            : t("updates.manualCheckDesc", {
                                defaultValue: "Check the selected channel now.",
                              })}
                  </div>
                </div>
              </div>
              {state?.currentVersion && <Badge variant="secondary">v{state.currentVersion}</Badge>}
            </div>

            {(hasUpdate || ready || downloading) && releaseNotes && (
              <div className="bg-muted/50 space-y-2 rounded-md border p-3">
                <div className="text-xs font-medium">
                  {t("updates.releaseNotes", { defaultValue: "What's new" })}
                </div>
                <div className="text-muted-foreground max-h-44 overflow-auto text-xs leading-5 break-words">
                  <ReactMarkdown
                    remarkPlugins={[remarkGfm, remarkEmoji]}
                    skipHtml
                    components={{
                      a: ({ href, children }) =>
                        isSafeMarkdownHref(href) ? (
                          <a
                            className="text-primary underline underline-offset-2"
                            href={href}
                            rel="noreferrer"
                            target="_blank"
                          >
                            {children}
                          </a>
                        ) : (
                          <>{children}</>
                        ),
                      blockquote: ({ children }) => (
                        <blockquote className="my-2 border-l-2 pl-3 italic">{children}</blockquote>
                      ),
                      code: ({ children }) => (
                        <code className="bg-muted rounded px-1 py-0.5 font-mono">{children}</code>
                      ),
                      h1: ({ children }) => (
                        <h4 className="text-foreground mb-2 font-semibold">{children}</h4>
                      ),
                      h2: ({ children }) => (
                        <h4 className="text-foreground mb-2 font-semibold">{children}</h4>
                      ),
                      h3: ({ children }) => (
                        <h5 className="text-foreground mb-2 font-semibold">{children}</h5>
                      ),
                      li: ({ children }) => <li className="my-1">{children}</li>,
                      ol: ({ children }) => <ol className="my-2 list-decimal pl-4">{children}</ol>,
                      p: ({ children }) => <p className="my-2 first:mt-0 last:mb-0">{children}</p>,
                      pre: ({ children }) => (
                        <pre className="bg-background my-2 overflow-auto rounded border p-2">
                          {children}
                        </pre>
                      ),
                      ul: ({ children }) => <ul className="my-2 list-disc pl-4">{children}</ul>,
                    }}
                  >
                    {renderedReleaseNotes}
                  </ReactMarkdown>
                </div>
              </div>
            )}

            {downloading && (
              <div className="space-y-2">
                <div className="bg-muted h-2 overflow-hidden rounded-full">
                  <div
                    className="bg-primary h-full transition-all"
                    style={{ width: `${progress}%` }}
                  />
                </div>
                <div className="text-muted-foreground text-xs">
                  {state.totalBytes
                    ? `${formatBytes(state.downloadedBytes)} / ${formatBytes(state.totalBytes)} (${progress}%)`
                    : formatBytes(state.downloadedBytes)}
                </div>
              </div>
            )}

            <div className="flex flex-wrap gap-2">
              <Button
                type="button"
                variant="outline"
                disabled={checking || downloading}
                onClick={() => {
                  void checkForAppUpdate(updateChannel).then(() =>
                    handleUpdateCheckComplete(Date.now())
                  )
                }}
              >
                {checking && <RefreshCw className="animate-spin" size={14} />}
                {t("updates.checkNow", { defaultValue: "Check now" })}
              </Button>

              {hasUpdate && (
                <Button
                  type="button"
                  disabled={downloading}
                  onClick={() => void downloadAndInstallAppUpdate(updateChannel)}
                >
                  <Download size={14} />
                  {t("updates.downloadInstall", { defaultValue: "Download and install" })}
                </Button>
              )}

              {ready && (
                <Button type="button" onClick={() => void relaunchApp()}>
                  <RotateCcw size={14} />
                  {t("updates.restart", { defaultValue: "Restart" })}
                </Button>
              )}

              {state?.status === "not-available" && (
                <span className="text-muted-foreground inline-flex items-center gap-1.5 text-xs">
                  <CheckCircle2 size={14} />
                  {t("updates.latest", { defaultValue: "Latest version" })}
                </span>
              )}

              {updateChannel === "beta-dev" && (
                <span className="text-muted-foreground inline-flex items-center gap-1.5 text-xs">
                  <Send size={14} />
                  {t("updates.betaHint", { defaultValue: "Preview builds may update more often." })}
                </span>
              )}
            </div>
          </CardContent>
        </Card>
      </div>
    </ScrollArea>
  )
}
