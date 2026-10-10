import React, { useCallback, useEffect, useMemo, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { save as saveFileDialog } from "@tauri-apps/plugin-dialog"
import { FileDown, FolderOpen, RefreshCw, ScrollText, Search, Trash2 } from "lucide-react"
import { useTranslation } from "react-i18next"

import { CodeMirrorEditor } from "@/components/CodeMirrorEditor"
import { LogReplayer } from "@/components/LogsPanel/LogReplayer"
import { useConfirmDialog } from "@/components/ui/app-dialog"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { useToast } from "@/hooks/use-toast"
import { formatLogSize } from "@/lib/terminalLogRecording"
import { cn, toErrorMessage } from "@/lib/utils"

/** A logged session, as `list_terminal_logs` reports it. */
export interface LogSession {
  id: string
  sessionType: string
  profile: string
  host: string
  port: number
  username: string
  startedAtMs: number
  modifiedAtMs: number
  rawBytes: number
  plainBytes: number
  hasRaw: boolean
  hasPlain: boolean
  recording: boolean
}

type LogView = "replay" | "text"

/** Refreshes after logging changes come in, at most this often. */
const REFRESH_DELAY_MS = 500

/** What the list calls a session: its connection, or `localLabel` for a local shell. */
export function logSessionTitle(session: LogSession, localLabel: string): string {
  if (session.sessionType !== "ssh") return localLabel
  return session.profile || session.host || session.id
}

/** Where an SSH session went; nothing for a local shell. */
export function logSessionTarget(session: LogSession): string | null {
  if (session.sessionType !== "ssh") return null
  const host =
    session.port && session.port !== 22 ? `${session.host}:${session.port}` : session.host
  return session.username ? `${session.username}@${host}` : host
}

export function matchesLogFilter(session: LogSession, filter: string): boolean {
  const query = filter.trim().toLowerCase()
  if (!query) return true
  return [session.profile, session.host, session.username, session.id].some((value) =>
    value.toLowerCase().includes(query)
  )
}

const LogTextView: React.FC<{ sessionId: string }> = ({ sessionId }) => {
  const [text, setText] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const { t } = useTranslation()

  // The panel remounts the view for another session, so this loads once.
  useEffect(() => {
    let disposed = false
    invoke<ArrayBuffer>("load_terminal_log_text", { id: sessionId })
      .then((buffer) => {
        if (!disposed) setText(new TextDecoder().decode(buffer))
      })
      .catch((reason) => {
        if (!disposed) setError(toErrorMessage(reason))
      })
    return () => {
      disposed = true
    }
  }, [sessionId])

  if (error) {
    return (
      <div role="alert" className="text-destructive p-6 text-sm">
        {error}
      </div>
    )
  }
  if (text === null) {
    return <p className="text-muted-foreground p-6 text-sm">{t("terminalLogs.loading")}</p>
  }
  return <CodeMirrorEditor className="h-full min-h-0" value={text} readOnly />
}

export const LogsPanel: React.FC = () => {
  const { t, i18n } = useTranslation()
  const { toast } = useToast()
  const { confirm, ConfirmDialog } = useConfirmDialog()
  const [sessions, setSessions] = useState<LogSession[] | null>(null)
  const [listError, setListError] = useState<string | null>(null)
  const [filter, setFilter] = useState("")
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [view, setView] = useState<LogView>("replay")

  const refresh = useCallback(async () => {
    try {
      setSessions(await invoke<LogSession[]>("list_terminal_logs"))
      setListError(null)
    } catch (error) {
      setListError(toErrorMessage(error))
    }
  }, [])

  useEffect(() => {
    let disposed = false
    let timer: number | undefined
    let unlisten: (() => void) | undefined
    // eslint-disable-next-line react-hooks/set-state-in-effect -- state is only set after await; false positive fixed upstream in facebook/react#36734
    void refresh()
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
      unlisten?.()
    }
  }, [refresh])

  const filtered = useMemo(
    () => (sessions ?? []).filter((session) => matchesLogFilter(session, filter)),
    [filter, sessions]
  )
  const selected = sessions?.find((session) => session.id === selectedId) ?? null
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
      defaultPath: `${session.id}.cast`,
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
      await refresh()
    } catch (error) {
      toast({
        title: t("terminalLogs.deleteFailed"),
        description: toErrorMessage(error),
        variant: "destructive",
      })
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
              <button
                key={session.id}
                type="button"
                role="option"
                aria-selected={session.id === selectedId}
                onClick={() => setSelectedId(session.id)}
                className={cn(
                  "hover:bg-muted/60 w-full rounded-md px-3 py-2 text-left",
                  session.id === selectedId && "bg-muted"
                )}
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
            ))}
          </div>
        </aside>

        <div className="flex min-w-0 flex-1 flex-col">
          {!selected ? (
            <div className="text-muted-foreground flex flex-1 items-center justify-center p-6 text-sm">
              {t("terminalLogs.selectSession")}
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
