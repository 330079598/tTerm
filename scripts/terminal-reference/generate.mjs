// Uses installed packages only, without open(), DOM, application startup or user data.
import assert from "node:assert/strict"
import { createRequire } from "node:module"
import { readFile, writeFile, mkdir } from "node:fs/promises"
import { fileURLToPath } from "node:url"
import xterm from "@xterm/xterm"
import serialization from "@xterm/addon-serialize"
import unicode from "@xterm/addon-unicode11"
import { fixtures } from "./fixtures.mjs"

const require = createRequire(import.meta.url)
for (const [name, version] of Object.entries({
  "@xterm/xterm": "6.0.0",
  "@xterm/addon-serialize": "0.14.0",
  "@xterm/addon-unicode11": "0.9.0",
})) {
  assert.equal(require(`${name}/package.json`).version, version, `${name} changed`)
}
const directory = new URL("../../crates/tterm-terminal/tests/fixtures/replay/", import.meta.url)
const check = process.argv.includes("--check")
assert(
  process.argv.slice(2).every((arg) => arg === "--check"),
  "usage: node scripts/terminal-reference/generate.mjs [--check]"
)
await mkdir(directory, { recursive: true })
const hex = (text) => Buffer.from(text).toString("hex") || "-"
const write = (terminal, bytes) => new Promise((resolve) => terminal.write(bytes, resolve))

function color(cell, prefix) {
  if (cell[`is${prefix}Default`]()) return "default"
  const value = cell[`get${prefix}Color`]()
  return cell[`is${prefix}RGB`]() ? `rgb:${value.toString(16).padStart(6, "0")}` : `index:${value}`
}

function screen(terminal, replies) {
  const buffer = terminal.buffer.active
  const lines = [
    `grid\t${terminal.cols}\t${terminal.rows}`,
    `cursor\t${buffer.cursorY}\t${buffer.cursorX}`,
    `screen\t${buffer.type}`,
  ]
  for (let row = 0; row < terminal.rows; row++) {
    const line = buffer.getLine(buffer.baseY + row)
    for (let col = 0; col < terminal.cols; col++) {
      const cell = line.getCell(col)
      const flags = [
        cell.isBold(),
        cell.isDim(),
        cell.isItalic(),
        cell.isInverse(),
        cell.isInvisible(),
        cell.isStrikethrough(),
      ]
        .map(Boolean)
        .map(Number)
        .join("")
      // Pinned xterm 6 internals supply metadata absent from its public buffer API.
      // SerializeAddon drops OSC 8 targets, so the sidecar must retain them.
      const link = terminal._core._oscLinkService.getLinkData(cell.extended.urlId)
      const underline = cell.getUnderlineStyle()
      const ulColor =
        underline && !cell.isUnderlineColorDefault()
          ? cell.isUnderlineColorRGB()
            ? `rgb:${cell.getUnderlineColor().toString(16).padStart(6, "0")}`
            : `index:${cell.getUnderlineColor()}`
          : "default"
      lines.push(
        [
          row,
          col,
          hex(cell.getChars() || (cell.getWidth() === 0 ? "" : " ")),
          cell.getWidth(),
          color(cell, "Fg"),
          color(cell, "Bg"),
          flags,
          underline,
          ulColor,
          hex(link?.id || ""),
          hex(link?.uri || ""),
        ].join("\t")
      )
    }
  }
  lines.push(`replies\t${hex(replies.join(""))}`)
  return lines.join("\n") + "\n"
}

function encode(frames) {
  return Buffer.concat(
    frames.map((frame) => {
      const header = Buffer.alloc(13)
      header[0] = frame.bytes ? 0 : 1
      header.writeBigUInt64LE(BigInt(frame.at), 1)
      if (frame.bytes) {
        header.writeUInt32LE(frame.bytes.length, 9)
        return Buffer.concat([header, frame.bytes])
      }
      header.writeUInt16LE(frame.cols, 9)
      header.writeUInt16LE(frame.rows, 11)
      return header
    })
  )
}

async function save(name, data) {
  const path = new URL(name, directory)
  const bytes = Buffer.from(data)
  assert(bytes.length < 32 * 1024, `${name} exceeds 32 KiB`)
  if (check) assert.deepEqual(await readFile(path), bytes, `${name} is stale`)
  else await writeFile(path, bytes)
}

for (const fixture of fixtures) {
  const terminal = new xterm.Terminal({
    cols: fixture.cols,
    rows: fixture.rows,
    scrollback: fixture.scrollback,
    allowProposedApi: true,
  })
  const addon = new serialization.SerializeAddon()
  terminal.loadAddon(new unicode.Unicode11Addon())
  terminal.unicode.activeVersion = "11"
  terminal.loadAddon(addon)
  const replies = []
  terminal.onData((data) => replies.push(data))
  try {
    for (const frame of fixture.frames) {
      if (frame.bytes) await write(terminal, frame.bytes)
      else terminal.resize(frame.cols, frame.rows)
    }
    await save(`${fixture.name}.bin`, encode(fixture.frames))
    await save(`${fixture.name}.ansi`, addon.serialize({ scrollback: 0 }))
    await save(`${fixture.name}.screen`, screen(terminal, replies))
    console.log(
      `${check ? "checked" : "generated"} ${fixture.name}: ${fixture.frames.length} frames, ${terminal.cols}x${terminal.rows}, unicode=11`
    )
  } finally {
    terminal.dispose()
  }
}
console.log(
  `reference: ${fixtures.length} fixtures; ${fileURLToPath(directory)}; no application or DOM opened`
)
