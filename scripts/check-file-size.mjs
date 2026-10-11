#!/usr/bin/env node
// Keeps source files small enough to read in one sitting. A file over its
// language's limit fails the check, unless scripts/file-size-baseline.json
// lists it: those were already over when the limit came in, and each may
// shrink but never grow back.
//
//   node scripts/check-file-size.mjs            check
//   node scripts/check-file-size.mjs --update   lower the baseline after files shrank
//
// --update only lowers or drops entries, never raises or adds one, so a
// listed file that has to grow is split instead. Tests and generated files
// are not checked.

import { execFileSync } from "node:child_process"
import { existsSync, readFileSync, writeFileSync } from "node:fs"
import { dirname, extname, join } from "node:path"
import { fileURLToPath } from "node:url"

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..")
const BASELINE = join(ROOT, "scripts", "file-size-baseline.json")

const LIMITS = { ".rs": 800, ".ts": 500, ".tsx": 500, ".css": 600 }
const EXEMPT = [/\.test\.tsx?$/, /(^|\/)tests\.rs$/, /_tests\.rs$/, /(^|\/)tests\//, /\.gen\.ts$/]

function sourceFiles() {
  const output = execFileSync(
    "git",
    ["ls-files", "--cached", "--others", "--exclude-standard", "--", "src", "src-tauri/src"],
    { cwd: ROOT, encoding: "utf8" }
  )
  return output
    .split("\n")
    .filter((path) => path && LIMITS[extname(path)] && !EXEMPT.some((re) => re.test(path)))
    .filter((path) => existsSync(join(ROOT, path)))
}

function lineCount(path) {
  const text = readFileSync(join(ROOT, path), "utf8")
  const lines = text.split("\n").length
  return text.endsWith("\n") ? lines - 1 : lines
}

function readBaseline() {
  return existsSync(BASELINE) ? JSON.parse(readFileSync(BASELINE, "utf8")) : {}
}

function writeBaseline(entries) {
  const sorted = Object.fromEntries(Object.entries(entries).sort(([a], [b]) => a.localeCompare(b)))
  writeFileSync(BASELINE, JSON.stringify(sorted, null, 2) + "\n")
}

const sizes = new Map(sourceFiles().map((path) => [path, lineCount(path)]))
const baseline = readBaseline()

if (process.argv.includes("--update")) {
  const next = {}
  for (const [path, allowed] of Object.entries(baseline)) {
    const lines = sizes.get(path)
    if (lines !== undefined && lines > LIMITS[extname(path)]) {
      next[path] = Math.min(allowed, lines)
    }
  }
  writeBaseline(next)
  console.log(`file-size baseline: ${Object.keys(next).length} file(s) still over the limit`)
  process.exit(0)
}

const failures = []
const lowerable = []
for (const [path, lines] of sizes) {
  const limit = LIMITS[extname(path)]
  const allowed = baseline[path]
  if (allowed === undefined) {
    if (lines > limit) {
      failures.push(`${path}: ${lines} lines, over the ${limit}-line limit; split it`)
    }
  } else if (lines > allowed) {
    failures.push(`${path}: ${lines} lines, grew past its baseline of ${allowed}; split it`)
  } else if (lines < allowed) {
    lowerable.push(path)
  }
}
for (const path of Object.keys(baseline)) {
  if (!sizes.has(path)) lowerable.push(path)
}

if (lowerable.length > 0) {
  failures.push(
    `${lowerable.length} baseline entr${lowerable.length === 1 ? "y is" : "ies are"} higher than ` +
      `the file now is (${lowerable.join(", ")}); run node scripts/check-file-size.mjs --update`
  )
}
if (failures.length > 0) {
  console.error(failures.join("\n"))
  process.exit(1)
}
console.log(`file-size: ${sizes.size} files within their limits`)
