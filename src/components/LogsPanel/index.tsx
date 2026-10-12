import React, { useCallback, useEffect, useMemo, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { save as saveFileDialog } from "@tauri-apps/plugin-dialog"
import { FileDown, FolderOpen, RefreshCw, ScrollText, Search, Trash2 } from "lucide-react"
import { useTranslation } from "react-i18next"

import {
  type LogSession,
  logSessionTitle,
  logSessionTarget,
  findTabLog,
  logFolder,
  logBaseName,
  idRange,
  matchesLogFilter,
} from "@/components/LogsPanel/logSessions"
import { LogTextView } from "@/components/LogsPanel/LogTextView"
import { LogReplayer } from "@/components/LogsPanel/LogReplayer"
import { useConfirmDialog } from "@/components/ui/app-dialog"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import { useToast } from "@/hooks/use-toast"
import { onLogsTabRequest, takeLogsTabRequest } from "@/lib/appNavigation"
import { formatLogSize } from "@/lib/terminalLogRecording"
import { cn, toErrorMessage } from "@/lib/utils"

type LogView = "replay" | "text"

/** What `delete_terminal_logs` did. */
interface DeleteReport {
  deleted: string[]
  failed: Array<{ id: string; message: string }>
}

/** Refreshes after logging changes come in, at most this often. */
const REFRESH_DELAY_MS = 500

export const LogsPanel: React.FC = () => {
  const { t, i18n } = useTranslation()
  const { toast } = useToast()
  const { confirm, ConfirmDialog } = useConfirmDialog()
  const [sessions, setSessions] = useState<LogSession[] | null>(null)
  const [listError, setListError] = useState<string | null>(null)
  const [filter, setFilter] = useState("")
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [view, setView] = useState<LogView>("replay")

  const [notice, setNotice] = useState<string | null>(null)
  /** Sessions ticked for deleting together. */
  const [checked, setChecked] = useState<ReadonlySet<string>>(() => new Set())
  const [checkAnchor, setCheckAnchor] = useState<string | null>(null)
  const [deleting, setDeleting] = useState(false)

  // The state is set in the promise callbacks rather than after an `await`,
  // which the effect lint can't yet tell apart from setting it synchronously.
  const refresh = useCallback(
    () =>
      invoke<LogSession[]>("list_terminal_logs")
        .then((list) => {
          setSessions(list)
          setListError(null)
          // A tab menu asked for a tab's logs: show its newest session.
          const request = takeLogsTabRequest()
          if (request === null) return
          return invoke<string[]>("terminal_log_ids_for_tab", { tabId: request.tabId }).then(
            (loggedIds) => {
              const newest = findTabLog(list, request, loggedIds)
              setFilter("")
              setSelectedId(newest?.id ?? null)
              setNotice(newest ? null : t("terminalLogs.noLogsForTab"))
            }
          )
        })
        .catch((error: unknown) => setListError(toErrorMessage(error))),
    [t]
  )

  useEffect(() => {
    let disposed = false
    let timer: number | undefined
    let unlisten: (() => void) | undefined
    void refresh()
    const stopRequests = onLogsTabRequest(() => void refresh())
    // A session starting or stopping a log changes the list.
    void listen("terminal-log-status", () => {
      window.clearTimeout(timer)
      timer = window.setTimeout(() => void refresh(), REFRESH_DELAY_MS)
    }).then((cleanup) => {
      if (disposed) cleanup()
      else unlisten = cleanup
    })
    return () => {
      disposed = true
      window.clearTimeout(timer)
      stopRequests()
      unlisten?.()
    }
  }, [refresh])

  const filtered = useMemo(
    () => (sessions ?? []).filter((session) => matchesLogFilter(session, filter)),
    [filter, sessions]
  )
  const selected = sessions?.find((session) => session.id === selectedId) ?? null
  // Only what the filter shows can be deleted together, and a session being
  // logged cannot be deleted at all.
  const checkable = useMemo(
    () => filtered.filter((session) => !session.recording).map((session) => session.id),
    [filtered]
  )
  const checkedShown = useMemo(
    () => filtered.filter((session) => checked.has(session.id) && !session.recording),
    [checked, filtered]
  )
  const allChecked = checkable.length > 0 && checkedShown.length === checkable.length
  // A session with only one kind of log shows that one.
  const shownView: LogView | null = !selected
    ? null
    : view === "replay" && selected.hasRaw
      ? "replay"
      : selected.hasPlain
        ? "text"
        : selected.hasRaw
          ? "replay"
          : null

  const exportCast = async (session: LogSession) => {
    const path = await saveFileDialog({
      defaultPath: `${logBaseName(session.id)}.cast`,
      filters: [{ name: "asciicast", extensions: ["cast"] }],
    }).catch(() => null)
    if (!path) return
    try {
      await invoke("export_terminal_log_asciicast", { id: session.id, path })
      toast({ title: t("terminalLogs.exported"), description: path })
    } catch (error) {
      toast({
        title: t("terminalLogs.exportFailed"),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    }
  }

  const reveal = async (session: LogSession) => {
    try {
      await invoke("reveal_terminal_log", { id: session.id })
    } catch (error) {
      toast({
        title: t("terminalLogs.revealFailed"),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    }
  }

  const remove = async (session: LogSession) => {
    const confirmed = await confirm({
      title: t("terminalLogs.deleteTitle"),
      description: t("terminalLogs.deleteDescription", {
        name: logSessionTitle(session, t("terminalLogs.local")),
      }),
      confirmText: t("common.delete"),
      cancelText: t("common.cancel"),
      variant: "destructive",
    })
    if (!confirmed) return
    try {
      await invoke("delete_terminal_log", { id: session.id })
      setSelectedId(null)
      setChecked((current) => new Set([...current].filter((id) => id !== session.id)))
      await refresh()
    } catch (error) {
      toast({
        title: t("terminalLogs.deleteFailed"),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    }
  }

  const toggleChecked = (id: string, extend: boolean) => {
    const ids = extend ? idRange(checkable, checkAnchor, id) : [id]
    const next = new Set(checked)
    const on = !checked.has(id)
    for (const each of ids) {
      if (on) next.add(each)
      else next.delete(each)
    }
    setChecked(next)
    setCheckAnchor(id)
  }

  const toggleAll = () => {
    setChecked(allChecked ? new Set() : new Set(checkable))
    setCheckAnchor(null)
  }

  const removeChecked = async () => {
    const targets = checkedShown
    if (targets.length === 0) return
    const confirmed = await confirm({
      title: t("terminalLogs.deleteManyTitle", { count: targets.length }),
      description: t("terminalLogs.deleteManyDescription", {
        size: formatLogSize(
          targets.reduce((total, session) => total + session.rawBytes + session.plainBytes, 0)
        ),
      }),
      confirmText: t("common.delete"),
      cancelText: t("common.cancel"),
      variant: "destructive",
    })
    if (!confirmed) return
    setDeleting(true)
    try {
      const report = await invoke<DeleteReport>("delete_terminal_logs", {
        ids: targets.map((session) => session.id),
      })
      const deleted = new Set(report.deleted)
      setChecked((current) => new Set([...current].filter((id) => !deleted.has(id))))
      if (selectedId !== null && deleted.has(selectedId)) setSelectedId(null)
      if (report.failed.length > 0) {
        toast({
          title: t("terminalLogs.deleteManyPartial", { count: report.failed.length }),
          description: report.failed[0].message,
          variant: "destructive",
        })
      } else {
        toast({ title: t("terminalLogs.deletedMany", { count: report.deleted.length }) })
      }
    } catch (error) {
      toast({
        title: t("terminalLogs.deleteFailed"),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    } finally {
      setDeleting(false)
      await refresh()
    }
  }

  const formatTime = (ms: number) =>
    ms > 0 ? new Date(ms).toLocaleString(i18n.language) : t("terminalLogs.unknownTime")

  return (
    <section className="flex h-full min-h-0 flex-col">
      <header className="border-border/80 flex items-start justify-between gap-4 border-b px-6 py-4">
        <div className="min-w-0">
          <h2 className="text-base font-semibold tracking-[-0.02em]">{t("terminalLogs.title")}</h2>
          <p className="text-muted-foreground mt-1 text-sm">{t("terminalLogs.description")}</p>
        </div>
        <Button type="button" variant="outline" onClick={() => void refresh()}>
          <RefreshCw />
          {t("terminalLogs.refresh")}
        </Button>
      </header>

      <div className="flex min-h-0 flex-1">
        <aside className="border-border/80 flex w-80 shrink-0 flex-col border-r">
          <div className="relative p-3">
            <Search className="text-muted-foreground pointer-events-none absolute top-1/2 left-5.5 size-4 -translate-y-1/2" />
            <Input
              value={filter}
              onChange={(event) => setFilter(event.target.value)}
              placeholder={t("terminalLogs.filter")}
              aria-label={t("terminalLogs.filter")}
              className="pl-8"
            />
          </div>
          {filtered.length > 0 && (
            <div className="flex items-center gap-2 px-5 pb-2">
              <Checkbox
                checked={allChecked}
                disabled={checkable.length === 0}
                onCheckedChange={toggleAll}
                aria-label={t("terminalLogs.selectAll")}
              />
              <span className="text-muted-foreground min-w-0 flex-1 truncate text-xs">
                {checkedShown.length > 0
                  ? t("terminalLogs.selectedCount", { count: checkedShown.length })
                  : t("terminalLogs.selectAll")}
              </span>
              <Button
                type="button"
                size="sm"
                variant="outline"
                disabled={checkedShown.length === 0 || deleting}
                onClick={() => void removeChecked()}
              >
                <Trash2 />
                {t("terminalLogs.deleteSelected")}
              </Button>
            </div>
          )}
          <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-3" role="listbox">
            {listError && (
              <p role="alert" className="text-destructive px-2 text-xs">
                {listError}
              </p>
            )}
            {sessions && filtered.length === 0 && (
              <div className="text-muted-foreground flex flex-col items-center gap-2 px-4 py-12 text-center text-sm">
                <ScrollText className="size-6" />
                <p>{sessions.length === 0 ? t("terminalLogs.empty") : t("terminalLogs.noMatch")}</p>
              </div>
            )}
            {filtered.map((session) => (
              <div
                key={session.id}
                className={cn(
                  "hover:bg-muted/60 flex items-start gap-2 rounded-md pl-3",
                  session.id === selectedId && "bg-muted"
                )}
              >
                <Checkbox
                  className="mt-2.5"
                  checked={checked.has(session.id) && !session.recording}
                  disabled={session.recording}
                  title={session.recording ? t("terminalLogs.deleteRecording") : undefined}
                  aria-label={t("terminalLogs.select", {
                    name: logSessionTitle(session, t("terminalLogs.local")),
                  })}
                  onClick={(event) => {
                    event.preventDefault()
                    toggleChecked(session.id, event.shiftKey)
                  }}
                />
                <button
                  type="button"
                  role="option"
                  aria-selected={session.id === selectedId}
                  onClick={() => {
                    setSelectedId(session.id)
                    setNotice(null)
                  }}
                  className="min-w-0 flex-1 py-2 pr-3 text-left"
                >
                  <div className="flex items-center gap-2">
                    <span className="min-w-0 flex-1 truncate text-sm font-medium">
                      {logSessionTitle(session, t("terminalLogs.local"))}
                    </span>
                    {session.recording && (
                      <Badge variant="destructive">{t("terminalLogs.recording")}</Badge>
                    )}
                  </div>
                  {logSessionTarget(session) && (
                    <div className="text-muted-foreground truncate text-xs">
                      {logSessionTarget(session)}
                    </div>
                  )}
                  <div className="text-muted-foreground mt-0.5 flex justify-between gap-2 text-xs tabular-nums">
                    <span className="truncate">{formatTime(session.startedAtMs)}</span>
                    <span>{formatLogSize(session.rawBytes + session.plainBytes)}</span>
                  </div>
                </button>
              </div>
            ))}
          </div>
        </aside>

        <div className="flex min-w-0 flex-1 flex-col">
          {!selected ? (
            <div className="text-muted-foreground flex flex-1 items-center justify-center p-6 text-sm">
              {notice ?? t("terminalLogs.selectSession")}
            </div>
          ) : (
            <>
              <div className="border-border/80 flex flex-wrap items-center justify-between gap-3 border-b px-4 py-3">
                <div className="min-w-0">
                  <div className="truncate text-sm font-semibold">
                    {logSessionTitle(selected, t("terminalLogs.local"))}
                  </div>
                  <div className="text-muted-foreground truncate text-xs">
                    {[
                      logSessionTarget(selected),
                      logFolder(selected.id),
                      formatTime(selected.startedAtMs),
                      formatLogSize(selected.rawBytes + selected.plainBytes),
                    ]
                      .filter(Boolean)
                      .join(" · ")}
                  </div>
                </div>
                <div className="flex flex-wrap items-center gap-2">
                  <div
                    className="border-border flex rounded-md border p-0.5"
                    role="group"
                    aria-label={t("terminalLogs.view")}
                  >
                    {(["replay", "text"] as const).map((option) => {
                      const available = option === "replay" ? selected.hasRaw : selected.hasPlain
                      return (
                        <Button
                          key={option}
                          type="button"
                          size="sm"
                          variant={shownView === option ? "secondary" : "ghost"}
                          disabled={!available}
                          aria-pressed={shownView === option}
                          title={available ? undefined : t(`terminalLogs.${option}Missing`)}
                          onClick={() => setView(option)}
                        >
                          {t(`terminalLogs.views.${option}`)}
                        </Button>
                      )
                    })}
                  </div>
                  <Button
                    type="button"
                    size="sm"
                    variant="outline"
                    disabled={!selected.hasRaw}
                    onClick={() => void exportCast(selected)}
                  >
                    <FileDown />
                    {t("terminalLogs.exportCast")}
                  </Button>
                  <Button
                    type="button"
                    size="sm"
                    variant="outline"
                    onClick={() => void reveal(selected)}
                  >
                    <FolderOpen />
                    {t("terminalLogs.reveal")}
                  </Button>
                  <Button
                    type="button"
                    size="sm"
                    variant="outline"
                    disabled={selected.recording}
                    title={selected.recording ? t("terminalLogs.deleteRecording") : undefined}
                    onClick={() => void remove(selected)}
                  >
                    <Trash2 />
                    {t("common.delete")}
                  </Button>
                </div>
              </div>
              <div className="min-h-0 flex-1">
                {shownView === "replay" && (
                  <LogReplayer key={selected.id} sessionId={selected.id} />
                )}
                {shownView === "text" && <LogTextView key={selected.id} sessionId={selected.id} />}
              </div>
            </>
          )}
        </div>
      </div>
      <ConfirmDialog />
    </section>
  )
}

export default LogsPanel
