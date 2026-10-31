#!/usr/bin/env node
// Regenerates every icon from the drawings in art.mjs: the app icon as
// app-icon.svg and app-icon.png, the mascot as mascot.svg (the About dialog)
// and mascot.png (the README), and the bundle icons in src-tauri/icons; and
// from crab.mjs, the parts of the crab that moves in src/assets/crab.
//
//   node scripts/app-icon/build.mjs
//
// The drawings are rendered to PNG with a headless Chrome or Edge, because
// the sticker filter comes out wrong in resvg. The script looks for one in
// the usual install locations; set TTERM_ICON_BROWSER to a browser binary to
// pick another. `tauri icon` then makes the bundle icons from the app icon, and
// icon.icns from the macOS version, which has a wider margin.

import { execFileSync, execSync } from "node:child_process"
import {
  copyFileSync,
  existsSync,
  mkdtempSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs"
import { tmpdir } from "node:os"
import { dirname, join } from "node:path"
import { fileURLToPath, pathToFileURL } from "node:url"
import { appIcon, macIcon, mascotIcon } from "./art.mjs"
import { writeCrabParts } from "./crab.mjs"

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..")
const ICONS_DIR = join(ROOT, "src-tauri", "icons")
const CRAB_DIR = join(ROOT, "src", "assets", "crab")
const MAC_FILE = "icon.icns"

const BROWSERS = {
  win32: [
    "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
    "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
    "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
    "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
  ],
  darwin: [
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
  ],
  linux: [
    "/usr/bin/google-chrome",
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
    "/usr/bin/microsoft-edge",
  ],
}

function findBrowser() {
  const chosen = process.env.TTERM_ICON_BROWSER
  if (chosen) {
    if (!existsSync(chosen))
      throw new Error(`TTERM_ICON_BROWSER points at ${chosen}, which does not exist.`)
    return chosen
  }
  const found = (BROWSERS[process.platform] ?? []).find((path) => existsSync(path))
  if (!found) {
    throw new Error(
      "No Chrome or Edge found. Install one, or set TTERM_ICON_BROWSER to a Chromium-based browser binary."
    )
  }
  return found
}

function renderPng(browser, svgPath, pngPath, workDir) {
  execFileSync(
    browser,
    [
      "--headless=new",
      "--disable-gpu",
      "--hide-scrollbars",
      "--default-background-color=00000000",
      "--window-size=1024,1024",
      `--user-data-dir=${join(workDir, "browser-profile")}`,
      `--screenshot=${pngPath}`,
      pathToFileURL(svgPath).href,
    ],
    { stdio: "ignore" }
  )
  if (!existsSync(pngPath)) throw new Error(`${browser} did not write ${pngPath}.`)
}

function tauriIcon(pngPath, outDir) {
  execSync(`pnpm tauri icon "${pngPath}" -o "${outDir}"`, { cwd: ROOT, stdio: "pipe" })
}

function build(workDir) {
  const browser = findBrowser()
  const sets = { app: appIcon(), mac: macIcon(), mascot: mascotIcon() }
  for (const [name, svg] of Object.entries(sets)) {
    writeFileSync(join(workDir, `${name}.svg`), svg)
    renderPng(browser, join(workDir, `${name}.svg`), join(workDir, `${name}.png`), workDir)
  }
  tauriIcon(join(workDir, "app.png"), join(workDir, "app"))
  tauriIcon(join(workDir, "mac.png"), join(workDir, "mac"))

  writeFileSync(join(ROOT, "app-icon.svg"), sets.app)
  copyFileSync(join(workDir, "app.png"), join(ROOT, "app-icon.png"))
  writeFileSync(join(ROOT, "mascot.svg"), sets.mascot)
  copyFileSync(join(workDir, "mascot.png"), join(ROOT, "mascot.png"))

  // Replace only the files the bundle already uses; `tauri icon` also makes
  // Android and iOS icons, which this app does not ship.
  for (const file of readdirSync(ICONS_DIR)) {
    const source = file === MAC_FILE ? "mac" : "app"
    copyFileSync(join(workDir, source, file), join(ICONS_DIR, file))
  }
  writeCrabParts(CRAB_DIR)
}

const workDir = mkdtempSync(join(tmpdir(), "tterm-icon-"))
try {
  build(workDir)
  console.log("app-icon, mascot, src-tauri/icons and src/assets/crab are up to date.")
} finally {
  rmSync(workDir, { recursive: true, force: true })
}
