// `tauri icon` writes the images of an .icns file in a different order on
// every run, so icon.icns changes in git each time the icons are regenerated
// even when no image does. Sorting the images by their type code makes the
// file the same whenever its images are.

import { readFileSync, writeFileSync } from "node:fs"

const HEADER = 8

/**
 * Rewrites the .icns file at `path` with its images sorted by type code. An
 * .icns file is the magic "icns" and the file's length, then one block per
 * image: a four-letter type code, the block's length and the image data.
 */
export function sortIcns(path) {
  const file = readFileSync(path)
  if (file.toString("latin1", 0, 4) !== "icns") throw new Error(`${path} is not an .icns file.`)
  const blocks = []
  for (let offset = HEADER; offset < file.length;) {
    const type = file.toString("latin1", offset, offset + 4)
    const length = file.readUInt32BE(offset + 4)
    if (length < HEADER || offset + length > file.length) {
      throw new Error(`${path} has a broken ${type} block at byte ${offset}.`)
    }
    // A table of contents lists the images in file order, and sorting would leave it stale.
    if (type === "TOC ")
      throw new Error(`${path} has a table of contents, which sorting would break.`)
    blocks.push({ type, data: file.subarray(offset, offset + length) })
    offset += length
  }
  blocks.sort((a, b) => (a.type < b.type ? -1 : a.type > b.type ? 1 : 0))
  writeFileSync(
    path,
    Buffer.concat([file.subarray(0, HEADER), ...blocks.map((block) => block.data)])
  )
}
