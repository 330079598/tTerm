// @vitest-environment jsdom
import { describe, it, expect, beforeEach, afterEach, vi } from "vitest"
import {
  initCanvasFontHost,
  restoreCanvasFontHost,
  safePreloadFont,
  MAX_HOST_CANVASES,
  HOST_ELEMENT_ID,
} from "@/lib/canvasFontHost"

describe("canvasFontHost", () => {
  beforeEach(() => {
    restoreCanvasFontHost()
  })

  afterEach(() => {
    restoreCanvasFontHost()
  })

  it("attaches newly created canvas elements to the hidden host container", () => {
    initCanvasFontHost()

    const canvas = document.createElement("canvas")
    const host = document.getElementById(HOST_ELEMENT_ID)

    expect(host).toBeTruthy()
    expect(canvas.parentElement).toBe(host)
    expect(canvas.isConnected).toBe(true)
  })

  it("does not attach non-canvas elements to the host container", () => {
    initCanvasFontHost()

    const div = document.createElement("div")
    const span = document.createElement("span")

    expect(div.parentElement).toBeNull()
    expect(span.parentElement).toBeNull()
  })

  it("allows canvas to be moved to an explicit parent via appendChild", () => {
    initCanvasFontHost()

    const canvas = document.createElement("canvas")
    const target = document.createElement("div")
    document.body.appendChild(target)

    target.appendChild(canvas)

    expect(canvas.parentElement).toBe(target)
    expect(canvas.isConnected).toBe(true)

    target.remove()
  })

  it("restores original document.createElement and removes host container", () => {
    initCanvasFontHost()
    expect(document.getElementById(HOST_ELEMENT_ID)).toBeTruthy()

    restoreCanvasFontHost()
    expect(document.getElementById(HOST_ELEMENT_ID)).toBeNull()

    const canvas = document.createElement("canvas")
    expect(canvas.parentElement).toBeNull()
  })

  it("enforces MAX_HOST_CANVASES bounded capacity to prevent DOM / memory leaks", () => {
    initCanvasFontHost()
    const host = document.getElementById(HOST_ELEMENT_ID)!
    expect(host).toBeTruthy()

    // Create more canvases than MAX_HOST_CANVASES
    const firstCanvas = document.createElement("canvas")
    for (let i = 1; i < MAX_HOST_CANVASES + 10; i++) {
      document.createElement("canvas")
    }

    // Capacity must be clamped at MAX_HOST_CANVASES
    expect(host.childElementCount).toBe(MAX_HOST_CANVASES)

    // The oldest un-accessed canvas should have been evicted
    expect(firstCanvas.parentElement).toBeNull()
    expect(firstCanvas.isConnected).toBe(false)
  })

  it("protects pinned xterm canvases from being evicted when capacity is reached", () => {
    initCanvasFontHost()
    const host = document.getElementById(HOST_ELEMENT_ID)!

    // Simulate an xterm-created canvas with pinned attribute
    const xtermCanvas = document.createElement("canvas")
    xtermCanvas.setAttribute("data-pinned-font-host", "true")
    xtermCanvas.getContext("2d")

    // Now flood with unpinned canvases up to and past MAX_HOST_CANVASES
    for (let i = 0; i < MAX_HOST_CANVASES + 5; i++) {
      document.createElement("canvas")
    }

    // Capacity is maintained
    expect(host.childElementCount).toBe(MAX_HOST_CANVASES)

    // xtermCanvas must NOT have been evicted because it was pinned
    expect(xtermCanvas.parentElement).toBe(host)
    expect(xtermCanvas.isConnected).toBe(true)
  })

  it("refreshes LRU position when getContext('2d') is invoked", () => {
    initCanvasFontHost()
    const host = document.getElementById(HOST_ELEMENT_ID)!

    const canvasA = document.createElement("canvas")
    const canvasB = document.createElement("canvas")

    // Initially canvasA is before canvasB
    expect(host.firstElementChild).toBe(canvasA)
    expect(host.lastElementChild).toBe(canvasB)

    // Touch canvasA by requesting 2d context
    canvasA.getContext("2d")

    // canvasA is now moved to the end of host children
    expect(host.lastElementChild).toBe(canvasA)
  })

  it("detaches canvas from host when requesting a WebGL context to prevent throttling", () => {
    initCanvasFontHost()
    const host = document.getElementById(HOST_ELEMENT_ID)!

    const canvas = document.createElement("canvas")
    expect(canvas.parentElement).toBe(host)

    // Requesting webgl should detach from the hidden host
    canvas.getContext("webgl")

    expect(canvas.parentElement).toBeNull()
    expect(canvas.isConnected).toBe(false)
  })

  describe("safePreloadFont", () => {
    it("returns false and does not throw for empty or malformed font strings", async () => {
      expect(await safePreloadFont(14, "")).toBe(false)
      expect(await safePreloadFont(14, "   ")).toBe(false)
      expect(await safePreloadFont(14, "Invalid\nFont")).toBe(false)
    })

    it("falls back to safe default size when size <= 0", async () => {
      const originalFonts = document.fonts
      const loadMock = vi.fn().mockResolvedValue([])
      try {
        Object.defineProperty(document, "fonts", {
          configurable: true,
          value: { load: loadMock },
        })

        const result = await safePreloadFont(0, "Monospace")
        expect(result).toBe(true)
        expect(loadMock).toHaveBeenCalledWith("14px Monospace")
      } finally {
        Object.defineProperty(document, "fonts", {
          configurable: true,
          value: originalFonts,
        })
      }
    })

    it("handles document.fonts.load rejection gracefully without throwing", async () => {
      const originalFonts = document.fonts
      try {
        // Mock document.fonts.load to reject
        Object.defineProperty(document, "fonts", {
          configurable: true,
          value: {
            load: vi.fn().mockRejectedValue(new Error("CSS Syntax Error")),
          },
        })

        const result = await safePreloadFont(14, "CorruptedFont")
        expect(result).toBe(false)
      } finally {
        Object.defineProperty(document, "fonts", {
          configurable: true,
          value: originalFonts,
        })
      }
    })

    it("returns true when document.fonts.load resolves", async () => {
      const originalFonts = document.fonts
      try {
        Object.defineProperty(document, "fonts", {
          configurable: true,
          value: {
            load: vi.fn().mockResolvedValue([]),
          },
        })

        const result = await safePreloadFont(14, "JetBrains Mono")
        expect(result).toBe(true)
      } finally {
        Object.defineProperty(document, "fonts", {
          configurable: true,
          value: originalFonts,
        })
      }
    })
  })
})
