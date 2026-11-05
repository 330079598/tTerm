/**
 * Source rewrites applied to the bundled `@xterm/addon-webgl` 0.19.0 module
 * (`lib/addon-webgl.mjs`) at build time. Each one matches the minified code of
 * that exact release and throws when it is missing, so an upgraded addon
 * fails the build instead of shipping without the fix.
 */

/** The module-level counter the atlas page version rewrite introduces. */
const VERSION_COUNTER = "__ttermAtlasPageVersion"

/**
 * The places that set an atlas page's `version`, in the order they appear:
 * merging pages, shifting pages after a delete, drawing a glyph, creating a
 * page and clearing one.
 */
const ATLAS_PAGE_VERSION_SITES: readonly (readonly [string, string])[] = [
  [
    "l=this._mergePages(o,a);l.version++;",
    `l=this._mergePages(o,a);l.version=++${VERSION_COUNTER};`,
  ],
  ["s.texturePage--;n.version++}", `s.texturePage--;n.version=++${VERSION_COUNTER}}`],
  ["_.addGlyph(m),_.version++,m}", `_.addGlyph(m),_.version=++${VERSION_COUNTER},m}`],
  [
    "this._glyphs=[];this.version=0;this.currentRow=",
    `this._glyphs=[];this.version=++${VERSION_COUNTER};this.currentRow=`,
  ],
  [
    "this.fixedRows.length=0,this.version++}",
    `this.fixedRows.length=0,this.version=++${VERSION_COUNTER}}`,
  ],
]

/**
 * Makes atlas page versions unique across all pages. The glyph atlas is
 * shared by every terminal with the same font and colors, and each terminal
 * re-uploads the texture at index `i` only when page `i`'s version differs
 * from the one it last uploaded. Versions were counted per page from zero,
 * and a page merge puts a different page at an index, often with a small
 * version that equals the old one; a terminal then keeps drawing from the old
 * texture with the new page's glyph positions, showing pieces of other glyphs.
 * xterm fixed it the same way after 0.19 (`AtlasPage.nextVersion` in the
 * 0.20 betas).
 */
export function patchAtlasPageVersions(code: string): string {
  let patched = code
  for (const [target, replacement] of ATLAS_PAGE_VERSION_SITES) {
    if (patched.split(target).length !== 2) {
      throw new Error(
        "@xterm/addon-webgl changed: re-check the atlas page version fix in src/lib/xtermWebglPatches.ts"
      )
    }
    patched = patched.replace(target, replacement)
  }
  // Textures start at version -1, so counting from 0 never matches one that
  // was never uploaded.
  return `let ${VERSION_COUNTER}=0;\n${patched}`
}
