import type { Terminal } from "@xterm/xterm"
import { describe, expect, it, vi } from "vitest"

import { recoverTerminalRenderer } from "@/components/TerminalTab/rendererRecovery"

function fixture() {
  const textures = [{ version: 4 }, { version: 7 }]
  const clear = vi.fn()
  const renderRows = vi.fn()
  const buffer = { active: { viewportY: 9, cursorX: 7, cursorY: 3 } }
  const term = {
    rows: 24,
    buffer,
    clear: vi.fn(),
    reset: vi.fn(),
    write: vi.fn(),
    resize: vi.fn(),
    refresh: vi.fn(),
    _core: {
      _renderService: {
        clear,
        _needsSelectionRefresh: false,
        _renderRows: renderRows,
        _renderer: {
          value: {
            clearTextureAtlas: vi.fn(),
            _glyphRenderer: { value: { _atlasTextures: textures } },
          },
        },
      },
    },
  }
  return { term, textures, clear, renderRows, buffer }
}

const recover = (term: object) => recoverTerminalRenderer(term as Terminal)

describe("recoverTerminalRenderer", () => {
  it("invalidates GPU texture versions and cell caches before drawing every row", () => {
    const { term, textures, clear, renderRows } = fixture()
    renderRows.mockImplementation(() => {
      expect(textures.map((texture) => texture.version)).toEqual([-1, -1])
      expect(clear).toHaveBeenCalledOnce()
      expect(term._core._renderService._needsSelectionRefresh).toBe(true)
    })
    recover(term)
    expect(renderRows).toHaveBeenCalledExactlyOnceWith(0, 23)
  })

  it("leaves the terminal data, shared atlas and PTY dimensions alone", () => {
    const { term, buffer } = fixture()
    recover(term)
    expect(term.buffer).toBe(buffer)
    expect(term.buffer.active).toEqual({ viewportY: 9, cursorX: 7, cursorY: 3 })
    for (const method of [term.clear, term.reset, term.write, term.resize]) {
      expect(method).not.toHaveBeenCalled()
    }
    expect(term._core._renderService._renderer.value.clearTextureAtlas).not.toHaveBeenCalled()
  })

  it("also clears canvas renderer caches without WebGL internals", () => {
    const clear = vi.fn()
    const renderRows = vi.fn()
    recover({ rows: 12, _core: { _renderService: { clear, _renderRows: renderRows } } })
    expect(clear).toHaveBeenCalledOnce()
    expect(renderRows).toHaveBeenCalledWith(0, 11)
  })

  it("falls back to public refresh if xterm internals are unavailable", () => {
    const refresh = vi.fn()
    recover({ rows: 12, refresh })
    expect(refresh).toHaveBeenCalledWith(0, 11)
  })
})
