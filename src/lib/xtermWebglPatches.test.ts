import { describe, expect, it } from "vitest"
// The addon's module as installed; the atlas plugin in vite.config.ts leaves
// `?raw` imports alone, so this is the unpatched code.
import addonSource from "@xterm/addon-webgl/lib/addon-webgl.mjs?raw"

import {
  patchAtlasLayoutRebuild,
  patchAtlasPageVersions,
  patchWebglAddon,
} from "@/lib/xtermWebglPatches"

describe("patchAtlasPageVersions", () => {
  it("leaves no per-page version counting in the installed addon", () => {
    const patched = patchAtlasPageVersions(addonSource)

    expect(patched).not.toMatch(/\.version\+\+/)
    expect(patched).not.toContain("this.version=0;")
    expect(patched.match(/version=\+\+__ttermAtlasPageVersion/g)).toHaveLength(5)
    expect(patched.startsWith("let __ttermAtlasPageVersion=0;\n")).toBe(true)
  })

  it("still reads textures by comparing page versions", () => {
    const patched = patchAtlasPageVersions(addonSource)

    expect(patched).toContain(
      "this._atlas.pages[r].version!==this._atlasTextures[r].version&&this._bindAtlasPageTexture"
    )
  })

  it("fails when the addon no longer has the code it rewrites", () => {
    expect(() => patchAtlasPageVersions("class AtlasPage{}")).toThrow(/atlas page version fix/)
  })
})

type FakeRenderer = {
  _glyphRenderer: { value: { beginFrame: () => boolean; _atlasTextures: { version: number }[] } }
  _charAtlas: object
  _terminal: { rows: number }
  _clearModel: (clearGlyphRenderer: boolean) => void
  _updateModel: (start: number, end: number) => void
}

/** The helper the layout rewrite puts on the module's first line. */
function rebuildAfterMerge(): (renderer: FakeRenderer) => void {
  const header = patchAtlasLayoutRebuild(addonSource).split("\n", 1)[0]
  return new Function(`${header};return __ttermRebuildAfterMerge`)() as (
    renderer: FakeRenderer
  ) => void
}

function fakeRenderer(layoutChanges: number) {
  let pending = layoutChanges
  const calls: string[] = []
  const textures = [{ version: 4 }, { version: 7 }]
  const renderer: FakeRenderer = {
    _glyphRenderer: {
      value: {
        beginFrame: () => pending-- > 0,
        _atlasTextures: textures,
      },
    },
    _charAtlas: {},
    _terminal: { rows: 30 },
    _clearModel: (clear) => calls.push(`clear ${clear}`),
    _updateModel: (start, end) => calls.push(`update ${start}-${end}`),
  }
  return { renderer, calls, textures }
}

describe("patchAtlasLayoutRebuild", () => {
  it("replaces the atlas flag that never resets with a layout counter", () => {
    const patched = patchAtlasLayoutRebuild(addonSource)

    expect(patched).not.toContain("_requestClearModel=!0")
    expect(patched.match(/_requestClearModel=\+\+__ttermAtlasLayout/g)).toHaveLength(3)
    expect(patched).toContain("e===this.__ttermLayoutSeen?!1:(this.__ttermLayoutSeen=e,!0)")
    expect(patched).toContain("this._atlas=t,this.__ttermLayoutSeen=void 0;")
  })

  it("redoes a frame after drawing its rows and before painting it", () => {
    const patched = patchAtlasLayoutRebuild(addonSource)

    expect(patched).toContain(
      ":this._updateModel(t,n),__ttermRebuildAfterMerge(this),this._rectangleRenderer.value.renderBackgrounds()"
    )
  })

  it("rebuilds every row and uploads every texture after a layout change", () => {
    const { renderer, calls, textures } = fakeRenderer(2)

    rebuildAfterMerge()(renderer)

    expect(calls).toEqual(["clear true", "update 0-29", "clear true", "update 0-29"])
    expect(textures.map((t) => t.version)).toEqual([-1, -1])
  })

  it("leaves a frame alone when the layout did not change", () => {
    const { renderer, calls, textures } = fakeRenderer(0)

    rebuildAfterMerge()(renderer)

    expect(calls).toEqual([])
    expect(textures.map((t) => t.version)).toEqual([4, 7])
  })

  it("stops rebuilding when the layout keeps changing", () => {
    const { renderer, calls } = fakeRenderer(Number.POSITIVE_INFINITY)

    rebuildAfterMerge()(renderer)

    expect(calls).toHaveLength(64)
  })

  it("fails when the addon no longer has the code it rewrites", () => {
    expect(() => patchAtlasLayoutRebuild("class TextureAtlas{}")).toThrow(
      /atlas layout rebuild fix/
    )
  })
})

describe("patchWebglAddon", () => {
  it("applies the page version and layout fixes together", () => {
    const patched = patchWebglAddon(addonSource)

    expect(patched.match(/version=\+\+__ttermAtlasPageVersion/g)).toHaveLength(5)
    expect(patched).toContain("__ttermRebuildAfterMerge(this)")
  })
})
