// @vitest-environment jsdom

import { act, cleanup, renderHook } from "@testing-library/react"
import type { Terminal } from "@xterm/xterm"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { useTerminalActivation } from "@/components/TerminalTab/useTerminalActivation"

vi.mock("@/hooks/useTerminalNotifications", () => ({ dismissTabToast: vi.fn() }))

function fixture(tabId = "terminal") {
  const textures = [{ version: 7 }]
  const service = {
    clear: vi.fn(),
    _renderRows: vi.fn(),
    _renderer: { value: { _glyphRenderer: { value: { _atlasTextures: textures } } } },
  }
  const term = {
    rows: 24,
    focus: vi.fn(),
    _core: { _renderService: service },
  }
  const options: Parameters<typeof useTerminalActivation>[0] = {
    tabId,
    isActive: true,
    isGlobalShortcutTarget: false,
    containerRef: { current: null },
    surfaceRef: { current: null },
    shellRef: { current: null },
    resizeObserverRef: { current: null },
    activateFitTimerRef: { current: null },
    resizeRafRef: { current: null },
    bellFlashTimerRef: { current: null },
    hasBeenActiveRef: { current: false },
    termRef: { current: term as unknown as Terminal },
    scheduleFitDuringResize: vi.fn(),
  }
  return { options, service, textures }
}

function resumeWindow() {
  act(() => {
    window.dispatchEvent(new Event("focus"))
    vi.advanceTimersByTime(20)
  })
}

describe("terminal activation recovery", () => {
  beforeEach(() => {
    vi.useFakeTimers()
    vi.spyOn(document, "visibilityState", "get").mockReturnValue("visible")
  })

  afterEach(() => {
    cleanup()
    vi.restoreAllMocks()
    vi.useRealTimers()
  })

  it("recovers a retained renderer when a background tab becomes active after resume", () => {
    const { options, service, textures } = fixture()
    const { rerender } = renderHook((props) => useTerminalActivation(props), {
      initialProps: options,
    })
    rerender({ ...options, isActive: false })
    service.clear.mockClear()
    service._renderRows.mockClear()
    textures[0].version = 9

    resumeWindow()
    expect(service.clear).not.toHaveBeenCalled()
    expect(textures[0].version).toBe(9)

    rerender(options)
    expect(textures[0].version).toBe(-1)
    expect(service.clear).toHaveBeenCalledOnce()
    expect(service._renderRows).toHaveBeenCalledExactlyOnceWith(0, 23)
  })

  it("recovers every visible pane on resume, but defers background panes until activation", () => {
    const first = fixture("first")
    const second = fixture("second")
    const background = fixture("background")
    renderHook(() => useTerminalActivation(first.options))
    renderHook(() => useTerminalActivation(second.options))
    const { rerender } = renderHook((props) => useTerminalActivation(props), {
      initialProps: { ...background.options, isActive: false },
    })
    first.service.clear.mockClear()
    second.service.clear.mockClear()

    resumeWindow()
    expect(first.service.clear).toHaveBeenCalledOnce()
    expect(second.service.clear).toHaveBeenCalledOnce()
    expect(background.service.clear).not.toHaveBeenCalled()

    rerender(background.options)
    expect(background.service.clear).toHaveBeenCalledOnce()
  })

  it("does not recover again on an unrelated render", () => {
    const { options, service } = fixture()
    const { rerender } = renderHook((props) => useTerminalActivation(props), {
      initialProps: options,
    })
    expect(service.clear).toHaveBeenCalledOnce()
    rerender({ ...options, isGlobalShortcutTarget: true })
    expect(service.clear).toHaveBeenCalledOnce()
  })

  it("uses the current terminal after a session replacement", () => {
    const first = fixture()
    const second = fixture()
    renderHook(() => useTerminalActivation(first.options))
    first.service.clear.mockClear()
    first.options.termRef.current = second.options.termRef.current

    resumeWindow()
    expect(first.service.clear).not.toHaveBeenCalled()
    expect(second.service.clear).toHaveBeenCalledOnce()
    expect(first.options.scheduleFitDuringResize).toHaveBeenCalled()
  })

  it("ignores resume while the terminal is unavailable", () => {
    const { options, service } = fixture()
    options.termRef.current = null
    renderHook(() => useTerminalActivation(options))
    resumeWindow()
    expect(service.clear).not.toHaveBeenCalled()
    expect(options.scheduleFitDuringResize).not.toHaveBeenCalled()
  })

  it("cancels a queued recovery when the last pane unmounts", () => {
    const { options, service } = fixture()
    const { unmount } = renderHook(() => useTerminalActivation(options))
    service.clear.mockClear()
    window.dispatchEvent(new Event("focus"))
    unmount()
    resumeWindow()
    expect(service.clear).not.toHaveBeenCalled()
    expect(vi.getTimerCount()).toBe(0)
  })
})
