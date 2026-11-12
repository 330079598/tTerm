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
  const patched = rewriteEach(code, ATLAS_PAGE_VERSION_SITES, "atlas page version fix")
  // Textures start at version -1, so counting from 0 never matches one that
  // was never uploaded.
  return `let ${VERSION_COUNTER}=0;\n${patched}`
}

/** The module-level counter the atlas layout rewrite introduces. */
const LAYOUT_COUNTER = "__ttermAtlasLayout"
/** The module-level function that redoes a frame an atlas merge cut through. */
const REBUILD_AFTER_MERGE = "__ttermRebuildAfterMerge"
/** The property a glyph renderer keeps the last atlas layout it built for in. */
const LAYOUT_SEEN = "__ttermLayoutSeen"

/**
 * The places the atlas layout rewrite changes, in the order they appear:
 * merging pages, clearing the atlas, adding the page for oversized glyphs,
 * the glyph renderer's frame start, the glyph renderer taking an atlas, and
 * the WebGL renderer drawing rows.
 */
const ATLAS_LAYOUT_SITES: readonly (readonly [string, string])[] = [
  [
    "this.pages.push(l),this._requestClearModel=!0,",
    `this.pages.push(l),this._requestClearModel=++${LAYOUT_COUNTER},`,
  ],
  [
    "this._cacheMapCombined.clear(),this._didWarmUp=!1}}_createNewPage(){",
    `this._cacheMapCombined.clear(),this._didWarmUp=!1,this._requestClearModel=++${LAYOUT_COUNTER}}}_createNewPage(){`,
  ],
  [
    "this.pages.push(this._overflowSizePage),this._requestClearModel=!0,",
    `this.pages.push(this._overflowSizePage),this._requestClearModel=++${LAYOUT_COUNTER},`,
  ],
  [
    "beginFrame(){return this._atlas?this._atlas.beginFrame():!0}",
    `beginFrame(){if(!this._atlas)return!0;let e=this._atlas.beginFrame();return e===this.${LAYOUT_SEEN}?!1:(this.${LAYOUT_SEEN}=e,!0)}`,
  ],
  ["setAtlas(t){this._atlas=t;for(", `setAtlas(t){this._atlas=t,this.${LAYOUT_SEEN}=void 0;for(`],
  [
    ":this._updateModel(t,n),this._rectangleRenderer.value.renderBackgrounds(),",
    `:this._updateModel(t,n),${REBUILD_AFTER_MERGE}(this),this._rectangleRenderer.value.renderBackgrounds(),`,
  ],
]

/**
 * Redoes a frame when the atlas changed its page layout while the frame was
 * being built, as the 0.20 betas do. Glyphs missing from the atlas are drawn
 * while the rows are walked, and the one that needs a page past the limit
 * merges four pages, which moves every glyph on them. The cells written
 * earlier in that frame keep the old page and position, and the frame is
 * drawn with them: pieces of other glyphs in other colors, until something
 * redraws the terminal (a selection, a focus change). Rebuilding every row
 * puts them on the new layout, and the textures are uploaded again because a
 * merge does not change the version of every page it touches.
 */
const REBUILD_AFTER_MERGE_SOURCE = `function ${REBUILD_AFTER_MERGE}(r){let g=r._glyphRenderer.value,m=!1;for(let i=0;i<32&&r._charAtlas&&g.beginFrame();i++)m=!0,r._clearModel(!0),r._updateModel(0,r._terminal.rows-1);if(m)for(let x of g._atlasTextures)x.version=-1}`

/**
 * Makes each glyph renderer notice every atlas layout change once, and
 * rebuilds the frame a layout change happened in. In 0.19 the atlas raises a
 * flag on its first page merge and never lowers it, so every later frame
 * rebuilds all rows but nothing notices a merge in the middle of one, and
 * clearing a shared atlas tells the other terminals nothing. A counter shared
 * by all atlases marks each layout change instead, and every glyph renderer
 * remembers the last one it built for.
 */
export function patchAtlasLayoutRebuild(code: string): string {
  const patched = rewriteEach(code, ATLAS_LAYOUT_SITES, "atlas layout rebuild fix")
  return `let ${LAYOUT_COUNTER}=0;${REBUILD_AFTER_MERGE_SOURCE}\n${patched}`
}

/** Applies every rewrite of the bundled addon this module defines. */
export function patchWebglAddon(code: string): string {
  return patchAtlasPageVersions(patchAtlasLayoutRebuild(code))
}

function rewriteEach(
  code: string,
  sites: readonly (readonly [string, string])[],
  fixName: string
): string {
  let patched = code
  for (const [target, replacement] of sites) {
    if (patched.split(target).length !== 2) {
      throw new Error(
        `@xterm/addon-webgl changed: re-check the ${fixName} in src/lib/xtermWebglPatches.ts`
      )
    }
    patched = patched.replace(target, replacement)
  }
  return patched
}
