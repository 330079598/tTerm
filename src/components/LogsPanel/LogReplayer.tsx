import React, { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { SearchAddon } from "@xterm/addon-search"
import { Unicode11Addon } from "@xterm/addon-unicode11"
import { Terminal } from "@xterm/xterm"
import { ChevronDown, ChevronUp, Pause, Play, RotateCcw, SkipForward } from "lucide-react"
import { useTranslation } from "react-i18next"

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
  framesPlayedBy,
  initialReplaySize,
  parseReplayFrames,
  replayDuration,
  type ReplayFrame,
} from "@/lib/terminalLogReplay"
import { toErrorMessage } from "@/lib/utils"

/** Pauses longer than this play as this long while idle time is skipped. */
const IDLE_LIMIT_MS = 2000
/** Output written to the terminal in one go while seeking. */
const WRITE_CHUNK_BYTES = 512 * 1024
const SPEEDS = [0.5, 1, 2, 4, 8] as const
/** How often the position shown under the player updates while playing. */
const POSITION_UPDATE_MS = 100

function concatBytes(chunks: Uint8Array[]): Uint8Array {
  if (chunks.length === 1) return chunks[0]
  const total = chunks.reduce((sum, chunk) => sum + chunk.length, 0)
  const joined = new Uint8Array(total)
  let offset = 0
  for (const chunk of chunks) {
    joined.set(chunk, offset)
    offset += chunk.length
  }
  return joined
}

/**
 * Writes frames `from..to` in order. A size change waits for the output
 * before it, since the terminal parses writes later than it resizes. Stops
 * early, returning false, once `alive` says the write is no longer wanted.
 */
async function writeFrames(
  term: Terminal,
  frames: readonly ReplayFrame[],
  from: number,
  to: number,
  alive: () => boolean
): Promise<boolean> {
  let pending: Uint8Array[] = []
  let pendingBytes = 0
  const flush = (after?: () => void) =>
    new Promise<void>((resolve) => {
      const data = pending.length > 0 ? concatBytes(pending) : new Uint8Array()
      pending = []
      pendingBytes = 0
      term.write(data, () => {
        if (alive()) after?.()
        resolve()
      })
    })

  for (let index = from; index < to; index++) {
    const frame = frames[index]
    if ("data" in frame) {
      pending.push(frame.data)
      pendingBytes += frame.data.length
      if (pendingBytes >= WRITE_CHUNK_BYTES) {
        await flush()
        if (!alive()) return false
      }
    } else {
      const { cols, rows } = frame
      await flush(() => term.resize(cols, rows))
      if (!alive()) return false
    }
  }
  if (pending.length > 0) await flush()
  return alive()
}

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
  const termRef = useRef<Terminal | null>(null)
  const searchRef = useRef<SearchAddon | null>(null)
  const [frames, setFrames] = useState<ReplayFrame[] | null>(null)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [playing, setPlaying] = useState(false)
  const [position, setPosition] = useState(0)
  const [speed, setSpeed] = useState(1)
  const [skipIdle, setSkipIdle] = useState(true)
  const [query, setQuery] = useState("")

  const timeline = useMemo(
    () => (frames ? buildReplayTimeline(frames, skipIdle ? IDLE_LIMIT_MS : null) : []),
    [frames, skipIdle]
  )
  const duration = replayDuration(timeline)

  // Playback state the animation loop reads and writes without re-rendering.
  const engine = useRef({
    index: 0,
    position: 0,
    playing: false,
    busy: false,
    generation: 0,
    lastReal: 0,
    lastShown: 0,
    raf: 0,
  })
  const framesRef = useRef<ReplayFrame[]>([])
  const timelineRef = useRef<number[]>([])
  const speedRef = useRef(speed)
  const paletteRef = useRef(palette)
  const configRef = useRef(config)

  useEffect(() => {
    speedRef.current = speed
  }, [speed])

  useEffect(() => {
    paletteRef.current = palette
    if (termRef.current) termRef.current.options.theme = palette
  }, [palette])

  useEffect(() => {
    configRef.current = config
    const term = termRef.current
    if (!term) return
    term.options.fontFamily = config.font_family
    term.options.fontSize = config.font_size
    term.options.lineHeight = config.terminal_line_height
    term.options.letterSpacing = config.terminal_letter_spacing
  }, [config])

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

  const stopLoop = useCallback(() => {
    const state = engine.current
    state.playing = false
    cancelAnimationFrame(state.raf)
  }, [])

  const tick = useCallback(function tick(now: number) {
    const state = engine.current
    const term = termRef.current
    if (!state.playing || !term) return
    const frames = framesRef.current
    const timeline = timelineRef.current
    const end = replayDuration(timeline)
    state.position += (now - state.lastReal) * speedRef.current
    state.lastReal = now

    if (!state.busy) {
      const target = framesPlayedBy(timeline, state.position)
      if (target > state.index) {
        const generation = state.generation
        const from = state.index
        state.busy = true
        state.index = target
        void writeFrames(term, frames, from, target, () => generation === state.generation).then(
          () => {
            if (generation === state.generation) state.busy = false
          }
        )
      }
    }

    if (state.index >= frames.length && !state.busy) {
      state.playing = false
      state.position = end
      setPlaying(false)
      setPosition(end)
      return
    }
    if (now - state.lastShown >= POSITION_UPDATE_MS) {
      state.lastShown = now
      setPosition(Math.min(state.position, end))
    }
    state.raf = requestAnimationFrame(tick)
  }, [])

  const startLoop = useCallback(() => {
    const state = engine.current
    state.playing = true
    state.lastReal = performance.now()
    setPlaying(true)
    cancelAnimationFrame(state.raf)
    state.raf = requestAnimationFrame(tick)
  }, [tick])

  /** Redraws the recording up to `targetMs`, then carries on if it was playing. */
  const seek = useCallback(
    async (targetMs: number, resume: boolean) => {
      const term = termRef.current
      if (!term) return
      const state = engine.current
      const generation = ++state.generation
      const alive = () => generation === state.generation
      stopLoop()
      state.busy = true
      const frames = framesRef.current
      const size = initialReplaySize(frames)
      // Reset through the write queue (ESC c): `term.reset()` would take
      // effect before writes still queued.
      await new Promise<void>((resolve) =>
        term.write("\x1bc", () => {
          if (alive()) term.resize(size.cols, size.rows)
          resolve()
        })
      )
      if (!alive()) return
      const target = framesPlayedBy(timelineRef.current, targetMs)
      if (!(await writeFrames(term, frames, 0, target, alive))) return
      state.busy = false
      state.index = target
      const end = replayDuration(timelineRef.current)
      state.position = Math.min(Math.max(targetMs, 0), end)
      setPosition(state.position)
      if (resume && target < frames.length) startLoop()
      else setPlaying(false)
    },
    [startLoop, stopLoop]
  )

  // A changed timeline (idle skipping toggled) keeps the frame and moves the clock.
  useEffect(() => {
    timelineRef.current = timeline
    const state = engine.current
    state.position = state.index > 0 ? (timeline[state.index - 1] ?? 0) : 0
    setPosition(Math.min(state.position, replayDuration(timeline)))
  }, [timeline])

  useEffect(() => {
    const container = containerRef.current
    if (!frames || !container) return
    framesRef.current = frames
    timelineRef.current = timeline
    const size = initialReplaySize(frames)
    const font = configRef.current
    const term = new Terminal({
      cols: size.cols,
      rows: size.rows,
      cursorBlink: false,
      disableStdin: true,
      scrollback: 100_000,
      fontFamily: font.font_family,
      fontSize: font.font_size,
      lineHeight: font.terminal_line_height,
      letterSpacing: font.terminal_letter_spacing,
      theme: paletteRef.current,
      allowProposedApi: true,
    })
    const search = new SearchAddon()
    term.loadAddon(search)
    term.loadAddon(new Unicode11Addon())
    term.unicode.activeVersion = "11"
    term.open(container)
    termRef.current = term
    searchRef.current = search
    const state = engine.current
    state.index = 0
    state.position = 0
    // Open on the whole recording, as a viewer; Play starts it over.
    void seek(Number.POSITIVE_INFINITY, false)

    return () => {
      state.generation++
      stopLoop()
      termRef.current = null
      searchRef.current = null
      term.dispose()
    }
    // The timeline at load time is enough here; later changes move the clock only.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [frames, seek, stopLoop])

  const togglePlay = () => {
    const state = engine.current
    if (state.playing) {
      stopLoop()
      setPlaying(false)
      setPosition(Math.min(state.position, duration))
    } else if (state.index >= framesRef.current.length) {
      void seek(0, true)
    } else {
      startLoop()
    }
  }

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
          onChange={(event) => void seek(Number(event.target.value), engine.current.playing)}
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
      <div className="min-h-0 flex-1 overflow-auto p-3" style={{ background: palette.background }}>
        {!frames && <p className="text-muted-foreground text-sm">{t("terminalLogs.loading")}</p>}
        <div ref={containerRef} className="inline-block" />
      </div>
    </div>
  )
}
