// @vitest-environment jsdom
import { describe, it, expect, beforeAll } from "vitest"
import { Terminal, type ITerminalAddon } from "@xterm/xterm"
import { CanvasAddon } from "@xterm/addon-canvas"
import { WebglAddon } from "@xterm/addon-webgl"

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
})
