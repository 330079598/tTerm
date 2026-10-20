import { useCallback, useEffect, useRef, useState, type RefObject } from "react"
import { SearchAddon } from "@xterm/addon-search"
import { Unicode11Addon } from "@xterm/addon-unicode11"
import { Terminal, type ITheme } from "@xterm/xterm"

import { fitReplayToViewport, type ReplayFontFit } from "@/components/LogsPanel/replayFit"
import { writeFrames } from "@/components/LogsPanel/replayWriter"
import type { AppConfig } from "@/lib/appConfig"
import { installOscLinkDedupe } from "@/lib/oscLinkDedupe"
import {
  framesPlayedBy,
  initialReplaySize,
  replayDuration,
  type ReplayFrame,
} from "@/lib/terminalLogReplay"

/** Set on the terminal's container while it has lines scrolled out of view. */
export const REPLAY_SCROLLBACK_CLASS = "replay-has-scrollback"

/** How often the position shown under the player updates while playing. */
const POSITION_UPDATE_MS = 100

interface ReplayPlayerOptions {
  containerRef: RefObject<HTMLDivElement | null>
  /** The panel the replay always fits inside. */
  viewportRef: RefObject<HTMLDivElement | null>
  frames: ReplayFrame[] | null
  timeline: number[]
  speed: number
  palette: ITheme
  config: AppConfig
  /** The font size the user set, used while `fitPanel` is off. */
  fontSize: number
  /** Draw at the largest size that fits the panel. */
  fitPanel: boolean
  /** Told each font size the replay is drawn at. */
  onFit: (fit: ReplayFontFit) => void
}

/**
 * The read-only terminal a recording plays in and the clock that drives it:
 * play, pause and seeking, at `speed`, along `timeline`.
 */
export function useReplayPlayer({
  containerRef,
  viewportRef,
  frames,
  timeline,
  speed,
  palette,
  config,
  fontSize,
  fitPanel,
  onFit,
}: ReplayPlayerOptions) {
  const termRef = useRef<Terminal | null>(null)
  const refitRef = useRef<(() => void) | null>(null)
  const searchRef = useRef<SearchAddon | null>(null)
  const [playing, setPlaying] = useState(false)
  const [position, setPosition] = useState(0)
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
  const fontSizeRef = useRef(fontSize)
  const onFitRef = useRef(onFit)

  useEffect(() => {
    onFitRef.current = onFit
  }, [onFit])

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
    // The font size is the replay's own, but the other font settings change it.
    term.options.fontFamily = config.font_family
    term.options.lineHeight = config.terminal_line_height
    term.options.letterSpacing = config.terminal_letter_spacing
    refitRef.current?.()
  }, [config])

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
    // `timelineRef` already holds the timeline: the effect above runs first.
    framesRef.current = frames
    const size = initialReplaySize(frames)
    const font = configRef.current
    const term = new Terminal({
      cols: size.cols,
      rows: size.rows,
      cursorBlink: false,
      disableStdin: true,
      scrollback: 100_000,
      fontFamily: font.font_family,
      fontSize: fontSizeRef.current,
      lineHeight: font.terminal_line_height,
      letterSpacing: font.terminal_letter_spacing,
      theme: paletteRef.current,
      allowProposedApi: true,
    })
    installOscLinkDedupe(term)
    const search = new SearchAddon()
    term.loadAddon(search)
    term.loadAddon(new Unicode11Addon())
    term.unicode.activeVersion = "11"
    term.open(container)
    termRef.current = term
    searchRef.current = search
    // xterm's scrollbar sits at the recording's right edge and is drawn even
    // with nothing to scroll, so the panel shows it only once there is.
    const updateScrollback = () =>
      container.classList.toggle(REPLAY_SCROLLBACK_CLASS, term.buffer.active.baseY > 0)
    const scrollbackListeners = [
      term.onWriteParsed(updateScrollback),
      term.onResize(updateScrollback),
    ]
    const state = engine.current
    state.index = 0
    state.position = 0
    // Open on the whole recording, as a viewer; Play starts it over.
    void seek(Number.POSITIVE_INFINITY, false)

    return () => {
      state.generation++
      stopLoop()
      for (const listener of scrollbackListeners) listener.dispose()
      container.classList.remove(REPLAY_SCROLLBACK_CLASS)
      termRef.current = null
      searchRef.current = null
      term.dispose()
    }
  }, [containerRef, frames, seek, stopLoop])

  // Declared after the terminal is made, so it runs on each new terminal too.
  useEffect(() => {
    fontSizeRef.current = fontSize
    const term = termRef.current
    const container = containerRef.current
    const viewport = viewportRef.current
    if (!term || !container || !viewport) return
    const request = { preferred: fontSize, fit: fitPanel }
    const fit = fitReplayToViewport(term, container, viewport, request, (fitted) =>
      onFitRef.current(fitted)
    )
    refitRef.current = fit.refit
    return () => {
      fit.dispose()
      refitRef.current = null
    }
  }, [containerRef, viewportRef, frames, fontSize, fitPanel])

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

  return {
    searchRef,
    playing,
    position,
    duration,
    isPlaying: () => engine.current.playing,
    togglePlay,
    seek,
  }
}
