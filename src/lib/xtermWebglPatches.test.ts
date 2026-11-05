import { describe, expect, it } from "vitest"
// The addon's module as installed; the atlas page version plugin in
// vite.config.ts leaves `?raw` imports alone, so this is the unpatched code.
import addonSource from "@xterm/addon-webgl/lib/addon-webgl.mjs?raw"

import { patchAtlasPageVersions } from "@/lib/xtermWebglPatches"

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
