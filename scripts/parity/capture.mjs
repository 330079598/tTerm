#!/usr/bin/env node
// Captures the reference side of the parity comparison: PNG screenshots and a
// layout JSON per screen, from the real React components running in WebKit.
//
//   node scripts/parity/capture.mjs [--out DIR] [--commit HASH] [--screen ID]
//
// `--out` defaults to target/parity/reference/<frontend commit>. The reference
// screenshots stay out of git and are cached by the last commit that touched
// the reference frontend; compare passes that hash to this script. The captured
// WebKit build comes from `pnpm exec playwright install webkit`; the pinned
// Playwright version and its browser revision live in docs/gpui-migration/parity/README.md.
//
// Vite is started from this script in `parity` mode on a free port, so a
// developer server on 1420 or 1423 is never touched.

import { execFileSync } from "node:child_process"
import { mkdir, readFile, writeFile } from "node:fs/promises"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"

import { webkit } from "playwright"
import { createServer } from "vite"

import { collectLayout } from "./layout.mjs"

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../..")
const MANIFEST_PATH = join(ROOT, "parity", "screens.json")
const FRONTEND_PATHS = [
  "src",
  "parity/screens.json",
  "parity/fixtures",
  "scripts/parity",
  "package.json",
  "pnpm-lock.yaml",
  "vite.config.ts",
  "tsconfig.json",
  "index.html",
  "parity.html",
]

async function main() {
  const options = parseArgs(process.argv.slice(2))
  const manifest = JSON.parse(await readFile(MANIFEST_PATH, "utf8"))
  const commit = options.commit ?? frontendCommit()
  const dirty = frontendDirty()
  const defaultName = dirty ? `${commit}-dirty` : commit
  const outDir = resolve(options.out ?? join(ROOT, "target", "parity", "reference", defaultName))
  if (dirty) {
    console.log(`reference: the frontend is dirty; capturing into ${outDir}`)
  }
  const screens = manifest.screens.filter(
    (screen) => options.screens.length === 0 || options.screens.includes(screen.id)
  )
  if (screens.length === 0) {
    throw new Error(
      `No screens matched; manifest ids: ${manifest.screens.map((screen) => screen.id).join(", ")}`
    )
  }

  await mkdir(outDir, { recursive: true })
  const server = await createServer({
    root: ROOT,
    configFile: join(ROOT, "vite.config.ts"),
    mode: "parity",
    logLevel: "error",
    server: { port: 0, strictPort: false, host: "127.0.0.1" },
  })

  let browser
  try {
    await server.listen()
    const base = server.resolvedUrls?.local?.[0]
    if (!base) throw new Error("Vite started without a local URL")
    browser = await launchBrowser()
    for (const screen of screens) {
      await captureScreen(browser, base, manifest, screen, outDir)
    }
    const output = {
      version: 2,
      commit,
      viewport: manifest.viewport,
      screens: screens.map((screen) => ({
        id: screen.id,
        image: `${screen.id}.png`,
        layout: `${screen.id}.layout.json`,
      })),
    }
    await writeFile(join(outDir, "manifest.json"), `${JSON.stringify(output, null, 2)}\n`)
    console.log(`reference: ${screens.length} screen(s) in ${outDir}`)
  } finally {
    if (browser) await browser.close()
    await server.close()
  }
}

async function captureScreen(browser, base, manifest, screen, outDir) {
  const { width, height, deviceScaleFactor } = manifest.viewport
  const context = await browser.newContext({
    viewport: { width, height },
    deviceScaleFactor,
    colorScheme: manifest.theme === "light" ? "light" : "dark",
    locale: "en-US",
    timezoneId: "UTC",
    reducedMotion: "reduce",
  })
  const pageErrors = []
  const page = await context.newPage()
  page.on("pageerror", (error) => pageErrors.push(String(error)))
  try {
    const url = new URL("parity.html", base)
    url.searchParams.set("screen", screen.id)
    url.searchParams.set("theme", manifest.theme)
    url.searchParams.set("scale", String(manifest.uiScalePercent))
    await page.goto(url.href, { waitUntil: "load" })
    await page.waitForFunction(() => window.__PARITY_READY__ === true, undefined, {
      timeout: 30_000,
    })
    await page.evaluate(async () => {
      await document.fonts.ready
    })
    const parityError = await page.evaluate(() => window.__PARITY_ERROR__ ?? null)
    if (parityError) throw new Error(parityError)
    if (pageErrors.length > 0) throw new Error(`page error: ${pageErrors[0]}`)
    await assertNoMissingFixtures(page)

    await page.screenshot({
      path: join(outDir, `${screen.id}.png`),
      animations: "disabled",
      caret: "hide",
    })
    await assertNoMissingFixtures(page)
    const layout = await page.evaluate(collectLayout, screen.id)
    await writeFile(
      join(outDir, `${screen.id}.layout.json`),
      `${JSON.stringify(layout, null, 2)}\n`
    )
    console.log(
      `${screen.id}: ${layout.elements.length} element(s), ${layout.textRegions.length} text region(s)`
    )
  } finally {
    await context.close()
  }
}

async function launchBrowser() {
  try {
    return await webkit.launch()
  } catch (error) {
    throw new Error(
      `Could not start Playwright's WebKit. Install it with:\n` +
        `  pnpm exec playwright install webkit\n` +
        `(${error instanceof Error ? error.message : String(error)})`
    )
  }
}

async function assertNoMissingFixtures(page) {
  const missing = await page.evaluate(() => window.__PARITY_MISSING__ ?? [])
  if (missing.length > 0) {
    throw new Error(`missing fixture(s) for: ${missing.join(", ")}`)
  }
}

function frontendDirty() {
  const output = execFileSync("git", ["status", "--porcelain", "--", ...FRONTEND_PATHS], {
    cwd: ROOT,
    encoding: "utf8",
  }).trim()
  return output.length > 0
}

function frontendCommit() {
  const output = execFileSync("git", ["log", "-1", "--format=%H", "--", ...FRONTEND_PATHS], {
    cwd: ROOT,
    encoding: "utf8",
  }).trim()
  if (!output) throw new Error("Could not read the reference frontend's last commit")
  return output
}

function parseArgs(args) {
  const options = { out: undefined, commit: undefined, screens: [] }
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index]
    const value = args[index + 1]
    if (argument === "--out" && value) {
      options.out = value
      index += 1
    } else if (argument === "--commit" && value) {
      options.commit = value
      index += 1
    } else if (argument === "--screen" && value) {
      options.screens.push(value)
      index += 1
    } else {
      throw new Error(`Unknown or incomplete argument: ${argument}`)
    }
  }
  return options
}

main().catch((error) => {
  console.error(`reference capture failed: ${error instanceof Error ? error.message : String(error)}`)
  process.exitCode = 1
})
