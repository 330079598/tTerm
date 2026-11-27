#!/usr/bin/env node
// Captures the first screen from the real GPUI window and the production
// WebKit components at the same viewport, scale, locale, data and selection.
//
//   node scripts/parity/first-screen.mjs
//   node scripts/parity/first-screen.mjs --skip-gpui
//   node scripts/parity/first-screen.mjs --skip-reference
//
// Evidence is written under target/parity/first-screen/. The native side
// launches disposable numbered instances under that directory and seeds them
// with synthetic configuration, groups and SSH profile metadata. Neither side
// opens a connection, reads a personal database or changes a system setting.

import { execFileSync } from "node:child_process"
import { createHash } from "node:crypto"
import { mkdir, readFile, rm, writeFile } from "node:fs/promises"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../..")
const OUTPUT = join(ROOT, "target", "parity", "first-screen")
const THEMES = ["dark", "light"]
const SCREENS = ["workspace.first-screen", "profiles.list.first-screen", "settings.terminal.font"]
const LIMITATIONS = [
  "The GPUI captures are real native windows, not desktop screen grabs. The harness seeds an 800x600 client area for workspace/profiles and 800x636 for settings, then crops the settings page below its 36 px title bar to 800x600.",
  "The reference side is Playwright WebKit with the production React title bar, profiles dialog and settings page. It is not a Tauri WKWebView window, so native window tint, blur and traffic lights are outside this comparison.",
  "Both sides use synthetic configuration, four synthetic saved SSH hosts and a local script with fixed output. No SSH connection, keychain read, personal database or system appearance change is involved.",
  "The profiles dialog is the product's own selected-first-row state on both sides. Hover is not captured.",
  "Cursor blink phases are not synchronized between clients. Pixel hashes can differ between captures.",
  "The saved-connections and settings pages have different layout implementations in the two clients; the captures compare the actual surfaces and do not force them into a shared clone.",
  "The workspace reference mounts the production TitleBarTabs, TitleBarActions and xterm surface. The broadcast and transfer tools have no populated state in this fixture, so their title-bar entries are not mounted there.",
  "The native launch helper currently writes a POSIX shell script; Windows needs an equivalent command before this entry can run there.",
]

const options = parseArgs(process.argv.slice(2))

if (options.skipGpui && options.skipReference) throw new Error("both capture sides were skipped")
await mkdir(OUTPUT, { recursive: true })
await rm(join(OUTPUT, "capture.json"), { force: true })
await rm(join(OUTPUT, "heatmaps"), { recursive: true, force: true })

const commit = gitCommit()
const result = {
  version: 1,
  commit,
  viewport: { width: 800, height: 600, deviceScaleFactor: 2 },
  locale: "en-US",
  uiStyle: "classic",
  uiScalePercent: 100,
  commands: {
    all: "node scripts/parity/first-screen.mjs",
    reference:
      "node scripts/parity/capture.mjs --theme <dark|light> --screen workspace.first-screen --screen profiles.list.first-screen --screen settings.terminal.font --out target/parity/first-screen/reference/<dark|light>",
    gpui: "cargo run --locked -p tterm-gpui --example first_screen_capture --features tterm-gpui/automation -- --theme <dark|light> --output target/parity/first-screen/gpui/<dark|light>",
  },
  themes: {},
  limitations: LIMITATIONS,
}

for (const theme of THEMES) {
  const referenceDir = join(OUTPUT, "reference", theme)
  const gpuiDir = join(OUTPUT, "gpui", theme)
  if (!options.skipReference) {
    await rm(referenceDir, { recursive: true, force: true })
    console.log(`first-screen: reference ${theme}`)
    execFileSync(
      process.execPath,
      [
        "scripts/parity/capture.mjs",
        "--theme",
        theme,
        ...SCREENS.flatMap((screen) => ["--screen", screen]),
        "--out",
        referenceDir,
      ],
      { cwd: ROOT, stdio: "inherit" }
    )
  }
  if (!options.skipGpui) {
    await rm(gpuiDir, { recursive: true, force: true })
    console.log(`first-screen: GPUI ${theme}`)
    execFileSync(
      "cargo",
      [
        "run",
        "--locked",
        "-p",
        "tterm-gpui",
        "--example",
        "first_screen_capture",
        "--features",
        "tterm-gpui/automation",
        "--",
        "--theme",
        theme,
        "--output",
        gpuiDir,
      ],
      { cwd: ROOT, stdio: "inherit" }
    )
  }
  result.themes[theme] = {
    reference: options.skipReference ? null : await describeFiles(referenceDir, SCREENS),
    gpui: options.skipGpui ? null : await describeFiles(gpuiDir, SCREENS),
  }
}

await writeFile(join(OUTPUT, "capture.json"), `${JSON.stringify(result, null, 2)}\n`)
await writeFile(join(OUTPUT, "README.md"), readme())
console.log(`first-screen: evidence in ${OUTPUT}`)

function parseArgs(args) {
  const parsed = { skipGpui: false, skipReference: false }
  for (const argument of args) {
    if (argument === "--skip-gpui") parsed.skipGpui = true
    else if (argument === "--skip-reference") parsed.skipReference = true
    else throw new Error(`unknown argument ${argument}`)
  }
  return parsed
}

function gitCommit() {
  try {
    return execFileSync("git", ["rev-parse", "HEAD"], { cwd: ROOT, encoding: "utf8" }).trim()
  } catch {
    return null
  }
}

async function describeFiles(directory, screens) {
  const files = {}
  for (const screen of screens) {
    const path = join(directory, `${screen}.png`)
    const bytes = await readFile(path)
    const { width, height } = pngSize(bytes)
    if (width !== 1600 || height !== 1200) throw new Error(`unexpected viewport: ${path}`)
    files[screen] = {
      path: relative(path),
      width,
      height,
      sha256: createHash("sha256").update(bytes).digest("hex"),
    }
  }
  return files
}

function pngSize(bytes) {
  if (bytes.length < 24 || bytes.toString("ascii", 12, 16) !== "IHDR") {
    throw new Error("not a PNG")
  }
  return { width: bytes.readUInt32BE(16), height: bytes.readUInt32BE(20) }
}

function relative(path) {
  return path.startsWith(ROOT) ? path.slice(ROOT.length + 1) : path
}

function readme() {
  return `# First-screen visual acceptance captures

Generated by \`node scripts/parity/first-screen.mjs\`.

- Viewport: 800x600 logical pixels at 2x (1600x1200 physical).
- Locale: en-US. Style: classic. Scale: 100%.
- GPUI: real native window through the debug automation endpoint. Workspace
  and profiles start in a disposable numbered instance at 800x600; settings
  starts at 800x636 and is cropped below the 36 px title bar to 800x600.
- Reference: production React components in Playwright WebKit, same viewport
  and device scale.
- Data: four synthetic SSH hosts, three synthetic groups, one local fixed
  output stream. No SSH connection, keychain read, personal database or
  system appearance change is involved.
- Selected state: the active local tab, the first saved host and the Terminal
  settings tab are selected on both sides.

Repeat a single side with:

\`\`\`sh
node scripts/parity/capture.mjs --theme dark \\
  --screen workspace.first-screen --screen profiles.list.first-screen \\
  --screen settings.terminal.font --out target/parity/first-screen/reference/dark

cargo run --locked -p tterm-gpui --example first_screen_capture \\
  --features tterm-gpui/automation -- \\
  --theme dark --output target/parity/first-screen/gpui/dark
\`\`\`

## Limitations

${LIMITATIONS.map((line) => `- ${line}`).join("\n")}
`
}
