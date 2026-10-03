#!/usr/bin/env node
// Rebuilds the theme library (src/assets/themes) from iTerm2-Color-Schemes,
// the collection Ghostty ships as `+list-themes`. It reads the repository's
// Ghostty files, which carry every scheme in one simple format.
//
//   node scripts/update-themes.mjs [ref]
//
// `ref` is a branch, tag or commit and defaults to master. The commit it
// resolves to is recorded in catalog.json so a build can be repeated.

import { mkdir, writeFile } from "node:fs/promises"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"
import { gunzipSync } from "node:zlib"

const REPOSITORY = "mbadolato/iTerm2-Color-Schemes"
const OUTPUT_DIR = join(dirname(fileURLToPath(import.meta.url)), "..", "src", "assets", "themes")
const HEX_COLOR = /^#[\da-f]{6}$/i

async function fetchOk(url, init) {
  const response = await fetch(url, init)
  if (!response.ok) {
    throw new Error(`${url}: ${response.status} ${response.statusText}`)
  }
  return response
}

async function resolveCommit(ref) {
  const response = await fetchOk(`https://api.github.com/repos/${REPOSITORY}/commits/${ref}`, {
    headers: { Accept: "application/vnd.github.sha" },
  })
  return (await response.text()).trim()
}

function readString(block, start, length) {
  const bytes = block.subarray(start, start + length)
  const end = bytes.indexOf(0)
  return bytes.subarray(0, end === -1 ? bytes.length : end).toString("utf8")
}

/** The regular files in a tar archive, with ustar, pax and GNU long names. */
function* tarEntries(archive) {
  let offset = 0
  let longName = null
  while (offset + 512 <= archive.length) {
    const header = archive.subarray(offset, offset + 512)
    if (header.every((byte) => byte === 0)) break
    const size = parseInt(readString(header, 124, 12).trim() || "0", 8)
    const type = String.fromCharCode(header[156] || 48)
    const body = archive.subarray(offset + 512, offset + 512 + size)
    offset += 512 + Math.ceil(size / 512) * 512

    if (type === "x") {
      const path = body.toString("utf8").match(/^\d+ path=(.*)$/m)
      if (path) longName = path[1]
      continue
    }
    if (type === "L") {
      longName = readString(body, 0, body.length)
      continue
    }
    if (type === "g") continue

    const prefix = readString(header, 345, 155)
    const name =
      longName ?? (prefix ? `${prefix}/${readString(header, 0, 100)}` : readString(header, 0, 100))
    longName = null
    if (type === "0") yield { name, body }
  }
}

/** A Ghostty theme file; `null` when a color is missing or malformed. */
export function parseGhosttyTheme(name, text) {
  const values = new Map()
  const palette = []
  for (const line of text.split(/\r?\n/)) {
    const match = line.match(/^\s*([\w-]+)\s*=\s*(.+?)\s*$/)
    if (!match) continue
    const [, key, value] = match
    if (key === "palette") {
      const entry = value.match(/^(\d+)\s*=\s*(#[\da-f]{6})$/i)
      if (entry && Number(entry[1]) < 16) palette[Number(entry[1])] = entry[2].toLowerCase()
    } else {
      values.set(key, value.toLowerCase())
    }
  }

  const color = (key) => (HEX_COLOR.test(values.get(key) ?? "") ? values.get(key) : undefined)
  const background = color("background")
  const foreground = color("foreground")
  if (!background || !foreground || palette.length !== 16 || palette.includes(undefined)) {
    return null
  }

  const theme = { name, background, foreground, palette }
  for (const [key, field] of [
    ["cursor-color", "cursor"],
    ["cursor-text", "cursorText"],
    ["selection-background", "selectionBackground"],
    ["selection-foreground", "selectionForeground"],
  ]) {
    if (color(key)) theme[field] = color(key)
  }
  return theme
}

/** One theme per line, so an update shows as a readable diff. */
function formatCatalog(catalog) {
  const themes = catalog.themes.map((theme) => `    ${JSON.stringify(theme)}`).join(",\n")
  return `{
  "source": ${JSON.stringify(catalog.source)},
  "commit": ${JSON.stringify(catalog.commit)},
  "themes": [
${themes}
  ]
}
`
}

async function main() {
  const commit = await resolveCommit(process.argv[2] ?? "master")
  console.log(`Downloading ${REPOSITORY}@${commit}`)
  const tarball = await fetchOk(`https://codeload.github.com/${REPOSITORY}/tar.gz/${commit}`)
  const archive = gunzipSync(Buffer.from(await tarball.arrayBuffer()))

  const themes = []
  const skipped = []
  const files = {}
  for (const { name, body } of tarEntries(archive)) {
    const [, ...rest] = name.split("/")
    const path = rest.join("/")
    if (path === "LICENSE" || path === "CREDITS.md") {
      files[path] = body
      continue
    }
    const themeName = path.match(/^ghostty\/([^/]+)$/)?.[1]
    if (!themeName) continue
    const theme = parseGhosttyTheme(themeName, body.toString("utf8"))
    if (theme) themes.push(theme)
    else skipped.push(themeName)
  }
  themes.sort((a, b) => a.name.localeCompare(b.name, "en", { sensitivity: "base" }))

  await mkdir(OUTPUT_DIR, { recursive: true })
  await writeFile(
    join(OUTPUT_DIR, "catalog.json"),
    formatCatalog({ source: `https://github.com/${REPOSITORY}`, commit, themes })
  )
  for (const [path, body] of Object.entries(files)) {
    await writeFile(join(OUTPUT_DIR, path), body)
  }

  console.log(`Wrote ${themes.length} themes to ${OUTPUT_DIR}`)
  if (skipped.length > 0) {
    console.warn(`Skipped ${skipped.length} incomplete themes: ${skipped.join(", ")}`)
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(error)
    process.exit(1)
  })
}
