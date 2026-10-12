import React, { useEffect, useMemo, useRef, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { ChevronDown, ChevronUp, Pause, Play, RotateCcw, SkipForward } from "lucide-react"
import { useTranslation } from "react-i18next"

import type { ReplayFontFit } from "@/components/LogsPanel/replayFit"
import { ReplayFontControls } from "@/components/LogsPanel/ReplayFontControls"
import {
  loadReplayViewPreferences,
  saveReplayViewPreferences,
  type ReplayViewPreferences,
} from "@/components/LogsPanel/replayView"
import { useReplayPlayer } from "@/components/LogsPanel/useReplayPlayer"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import { useConfig } from "@/contexts/ConfigContext"
import { useTheme } from "@/contexts/ThemeContext"
import { isImeKeyEvent } from "@/lib/ime"
import {
  buildReplayTimeline,
  formatReplayTime,
  parseReplayFrames,
  type ReplayFrame,
} from "@/lib/terminalLogReplay"
import { cn, toErrorMessage } from "@/lib/utils"

/** Pauses longer than this play as this long while idle time is skipped. */
const IDLE_LIMIT_MS = 2000
const SPEEDS = [0.5, 1, 2, 4, 8] as const

interface LogReplayerProps {
  sessionId: string
}

/** Plays a raw terminal log back in a read-only terminal. */
export const LogReplayer: React.FC<LogReplayerProps> = ({ sessionId }) => {
  const { t } = useTranslation()
  const { config } = useConfig()
  const { displayedTheme, getTheme } = useTheme()
  const palette = getTheme(displayedTheme)?.terminal ?? getTheme("default")!.terminal

  const containerRef = useRef<HTMLDivElement>(null)
  const viewportRef = useRef<HTMLDivElement>(null)
  const [frames, setFrames] = useState<ReplayFrame[] | null>(null)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [speed, setSpeed] = useState(1)
  const [skipIdle, setSkipIdle] = useState(true)
  const [query, setQuery] = useState("")
  const [view, setView] = useState(loadReplayViewPreferences)
  const [fitted, setFitted] = useState<ReplayFontFit | null>(null)
  const fontSize = view.fontSize ?? config.font_size

  const updateView = (change: Partial<ReplayViewPreferences>) => {
    const next = { ...view, ...change }
    setView(next)
    saveReplayViewPreferences(next)
  }

  const timeline = useMemo(
    () => (frames ? buildReplayTimeline(frames, skipIdle ? IDLE_LIMIT_MS : null) : []),
    [frames, skipIdle]
  )

  // The panel remounts the player for another session, so this loads once.
  useEffect(() => {
    let disposed = false
    invoke<ArrayBuffer>("load_terminal_log_recording", { id: sessionId })
      .then((buffer) => {
        if (!disposed) setFrames(parseReplayFrames(buffer))
      })
      .catch((error) => {
        if (!disposed) setLoadError(toErrorMessage(error))
      })
    return () => {
      disposed = true
    }
  }, [sessionId])

  const { searchRef, playing, position, duration, isPlaying, togglePlay, seek } = useReplayPlayer({
    containerRef,
    viewportRef,
    frames,
    timeline,
    speed,
    palette,
    config,
    fontSize,
    fitPanel: view.fitPanel,
    onFit: setFitted,
  })

  const find = (previous: boolean) => {
    const search = searchRef.current
    if (!search || !query) return
    if (previous) search.findPrevious(query)
    else search.findNext(query)
  }

  if (loadError) {
    return (
      <div role="alert" className="text-destructive p-6 text-sm">
        {loadError}
      </div>
    )
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="border-border flex flex-wrap items-center gap-2 border-b px-3 py-2">
        <Button
          type="button"
          size="icon"
          variant="outline"
          onClick={togglePlay}
          disabled={!frames}
          aria-label={playing ? t("terminalLogs.pause") : t("terminalLogs.play")}
          title={playing ? t("terminalLogs.pause") : t("terminalLogs.play")}
        >
          {playing ? <Pause /> : <Play />}
        </Button>
        <Button
          type="button"
          size="icon"
          variant="outline"
          onClick={() => void seek(0, true)}
          disabled={!frames}
          aria-label={t("terminalLogs.restart")}
          title={t("terminalLogs.restart")}
        >
          <RotateCcw />
        </Button>
        <Button
          type="button"
          size="icon"
          variant="outline"
          onClick={() => void seek(Number.POSITIVE_INFINITY, false)}
          disabled={!frames}
          aria-label={t("terminalLogs.skipToEnd")}
          title={t("terminalLogs.skipToEnd")}
        >
          <SkipForward />
        </Button>
        <input
          type="range"
          min={0}
          max={Math.max(duration, 1)}
          step={100}
          value={Math.min(position, duration)}
          disabled={!frames}
          onChange={(event) => void seek(Number(event.target.value), isPlaying())}
          aria-label={t("terminalLogs.position")}
          className="accent-primary min-w-32 flex-1"
        />
        <span className="text-muted-foreground font-mono text-xs tabular-nums">
          {formatReplayTime(position)} / {formatReplayTime(duration)}
        </span>
        <div className="w-24">
          <Select
            value={String(speed)}
            onChange={(event) => setSpeed(Number(event.target.value))}
            aria-label={t("terminalLogs.speed")}
          >
            {SPEEDS.map((value) => (
              <option key={value} value={value}>
                {value}×
              </option>
            ))}
          </Select>
        </div>
        <ReplayFontControls
          fontSize={fitted?.fontSize ?? fontSize}
          capped={fitted?.capped ?? false}
          fitPanel={view.fitPanel}
          onFontSizeChange={(next) => updateView({ fontSize: next })}
          onFitPanelChange={(fitPanel) => updateView({ fitPanel })}
        />
        <div className="flex items-center gap-1.5">
          <Checkbox id="replay-skip-idle" checked={skipIdle} onCheckedChange={setSkipIdle} />
          <Label
            htmlFor="replay-skip-idle"
            className="text-xs font-normal"
            title={t("terminalLogs.skipIdleDesc", { seconds: IDLE_LIMIT_MS / 1000 })}
          >
            {t("terminalLogs.skipIdle")}
          </Label>
        </div>
        <div className="flex items-center gap-1">
          <Input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={(event) => {
              if (event.key !== "Enter" || isImeKeyEvent(event.nativeEvent)) return
              event.preventDefault()
              find(event.shiftKey)
            }}
            placeholder={t("terminalLogs.search")}
            aria-label={t("terminalLogs.search")}
            className="h-8 w-40"
          />
          <Button
            type="button"
            size="icon"
            variant="ghost"
            onClick={() => find(true)}
            disabled={!query}
            aria-label={t("terminalLogs.findPrevious")}
            title={t("terminalLogs.findPrevious")}
          >
            <ChevronUp />
          </Button>
          <Button
            type="button"
            size="icon"
            variant="ghost"
            onClick={() => find(false)}
            disabled={!query}
            aria-label={t("terminalLogs.findNext")}
            title={t("terminalLogs.findNext")}
          >
            <ChevronDown />
          </Button>
        </div>
      </div>
      <div
        ref={viewportRef}
        className="min-h-0 flex-1 overflow-auto p-3"
        style={{ background: palette.background }}
      >
        {!frames && <p className="text-muted-foreground text-sm">{t("terminalLogs.loading")}</p>}
        {/* At least as wide as the panel, so xterm's scrollbar sits at its right edge. */}
        <div
          ref={containerRef}
          className={cn(
            // Clipped, so nothing xterm positions inside can scroll the panel.
            "w-max min-w-full overflow-hidden",
            // Keep in step with REPLAY_SCROLLBACK_CLASS in useReplayPlayer.
            "[&:not(.replay-has-scrollback)_.xterm-scrollable-element>.scrollbar.vertical]:hidden!"
          )}
        />
      </div>
    </div>
  )
}
