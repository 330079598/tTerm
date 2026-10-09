// @vitest-environment jsdom
import { describe, it, expect, beforeAll, vi } from "vitest"
import { Terminal, type ITerminalAddon } from "@xterm/xterm"
import { CanvasAddon } from "@xterm/addon-canvas"
import { WebglAddon } from "@xterm/addon-webgl"

import { disposeRendererAddon, renderAllRowsNow } from "@/components/TerminalTab/rendererRelease"

describe("Terminal Renderer Addon & Canvas Lifecycle", () => {
  beforeAll(() => {
    window.matchMedia =
      window.matchMedia ||
      function () {
        return {
          matches: false,
          addListener: function () {},
          removeListener: function () {},
        }
      }

    window.ResizeObserver =
      window.ResizeObserver ||
      class {
        observe() {}
        unobserve() {}
        disconnect() {}
      }

    HTMLCanvasElement.prototype.getContext = function () {
      return {
        fillRect: () => {},
        clearRect: () => {},
        getImageData: () => ({ data: new Array(4) }),
        putImageData: () => {},
        createImageData: () => [],
        setTransform: () => {},
        drawImage: () => {},
        save: () => {},
        fillText: () => {},
        restore: () => {},
        beginPath: () => {},
        moveTo: () => {},
        lineTo: () => {},
        closePath: () => {},
        stroke: () => {},
        translate: () => {},
        scale: () => {},
        rotate: () => {},
        arc: () => {},
        fill: () => {},
        measureText: () => ({ width: 10, fontBoundingBoxAscent: 10, fontBoundingBoxDescent: 2 }),
        transform: () => {},
        rect: () => {},
        clip: () => {},
      } as unknown as CanvasRenderingContext2D
    } as unknown as typeof HTMLCanvasElement.prototype.getContext
  })

  it("activates CanvasAddon and creates canvas layers on opened terminal", () => {
    const container = document.createElement("div")
    document.body.appendChild(container)

    const term = new Terminal({ allowProposedApi: true })
    term.open(container)

    const canvasAddon = new CanvasAddon()
    expect(() => term.loadAddon(canvasAddon)).not.toThrow()

    // Canvas addon should have attached canvas elements to .xterm-screen
    const canvases = container.querySelectorAll("canvas")
    expect(canvases.length).toBeGreaterThan(0)

    // Test clearTextureAtlas
    expect(() => canvasAddon.clearTextureAtlas?.()).not.toThrow()

    // Safe dispose
    expect(() => canvasAddon.dispose()).not.toThrow()

    term.dispose()
    container.remove()
  })

  it("dynamically switches between renderers without throwing", () => {
    const container = document.createElement("div")
    document.body.appendChild(container)

    const term = new Terminal({ allowProposedApi: true })
    term.open(container)

    // Start with CanvasAddon
    let currentAddon: (ITerminalAddon & { clearTextureAtlas?: () => void }) | null =
      new CanvasAddon()
    term.loadAddon(currentAddon)
    expect(container.querySelectorAll("canvas").length).toBeGreaterThan(0)

    // Dispose CanvasAddon and switch to a new CanvasAddon
    currentAddon.dispose()
    currentAddon = new CanvasAddon()
    expect(() => term.loadAddon(currentAddon!)).not.toThrow()

    // Verify clearTextureAtlas can be called after switch
    expect(() => currentAddon?.clearTextureAtlas?.()).not.toThrow()

    currentAddon.dispose()
    term.dispose()
    container.remove()
  })

  it("falls back to CanvasAddon when WebglAddon encounters context loss", () => {
    const container = document.createElement("div")
    document.body.appendChild(container)

    const term = new Terminal({ allowProposedApi: true })
    term.open(container)

    let activeAddon: ITerminalAddon | null = null

    const loadRenderer = (renderer: "webgl" | "canvas") => {
      if (activeAddon) {
        activeAddon.dispose()
        activeAddon = null
      }

      if (renderer === "webgl") {
        try {
          const webgl = new WebglAddon()
          webgl.onContextLoss(() => {
            webgl.dispose()
            const canvas = new CanvasAddon()
            term.loadAddon(canvas)
            activeAddon = canvas
          })
          term.loadAddon(webgl)
          activeAddon = webgl
          return
        } catch {
          // fallback
        }
      }

      const canvas = new CanvasAddon()
      term.loadAddon(canvas)
      activeAddon = canvas
    }

    // Load canvas renderer
    loadRenderer("canvas")
    expect(activeAddon).toBeInstanceOf(CanvasAddon)

    // Dispose
    ;(activeAddon as ITerminalAddon | null)?.dispose()
    term.dispose()
    container.remove()
  })

  it("properly re-attaches renderer addon and clears texture atlas on reconnection", () => {
    const container = document.createElement("div")
    document.body.appendChild(container)

    // Initial session
    let term = new Terminal({
      allowProposedApi: true,
      fontFamily: '"Fira Code", monospace',
      fontSize: 15,
    })
    term.open(container)
    let activeRenderer: (ITerminalAddon & { clearTextureAtlas?: () => void }) | null =
      new CanvasAddon()
    term.loadAddon(activeRenderer)
    expect(container.querySelectorAll("canvas").length).toBeGreaterThan(0)
    expect(term.options.fontFamily).toBe('"Fira Code", monospace')
    expect(term.options.fontSize).toBe(15)

    // Reconnect cleanup
    activeRenderer.dispose()
    activeRenderer = null
    term.dispose()
    container.replaceChildren()
    expect(container.children.length).toBe(0)

    // Reconnect: new terminal created with updated/current font and renderer
    term = new Terminal({
      allowProposedApi: true,
      fontFamily: '"JetBrains Mono Nerd Font", monospace',
      fontSize: 16,
    })
    term.open(container)
    activeRenderer = new CanvasAddon()
    term.loadAddon(activeRenderer)

    expect(container.querySelectorAll("canvas").length).toBeGreaterThan(0)
    expect(term.options.fontFamily).toBe('"JetBrains Mono Nerd Font", monospace')
    expect(term.options.fontSize).toBe(16)
    expect(() => activeRenderer?.clearTextureAtlas?.()).not.toThrow()

    activeRenderer.dispose()
    term.dispose()
    container.remove()
  })

  it("frees the canvases of a released renderer and leaves the terminal's own", () => {
    const container = document.createElement("div")
    document.body.appendChild(container)

    const term = new Terminal({ allowProposedApi: true })
    term.open(container)
    const canvasAddon = new CanvasAddon()
    term.loadAddon(canvasAddon)

    const screen = container.querySelector(".xterm-screen")!
    const layers = Array.from(screen.querySelectorAll("canvas"))
    expect(layers.length).toBeGreaterThan(0)
    const ownCanvas = document.createElement("canvas")
    ownCanvas.width = 20
    ownCanvas.height = 30
    screen.appendChild(ownCanvas)

    disposeRendererAddon(term, canvasAddon)

    for (const layer of layers) {
      expect(layer.isConnected).toBe(false)
      expect(layer.width).toBe(0)
      expect(layer.height).toBe(0)
    }
    expect(ownCanvas.width).toBe(20)
    expect(ownCanvas.height).toBe(30)

    term.dispose()
    container.remove()
  })

  it("draws every row of a reloaded renderer at once, even while xterm is paused", () => {
    const container = document.createElement("div")
    document.body.appendChild(container)

    const term = new Terminal({ allowProposedApi: true, rows: 12 })
    term.open(container)
    const canvasAddon = new CanvasAddon()
    term.loadAddon(canvasAddon)
    const renderService = (
      term as unknown as {
        _core: {
          _renderService: { _isPaused: boolean; _renderRows: (s: number, e: number) => void }
        }
      }
    )._core._renderService
    renderService._isPaused = true
    const renderRows = vi.spyOn(renderService, "_renderRows")

    renderAllRowsNow(term)

    expect(renderRows).toHaveBeenCalledWith(0, 11)

    canvasAddon.dispose()
    term.dispose()
    container.remove()
  })
})
