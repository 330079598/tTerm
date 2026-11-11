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

import { chromium, webkit } from "playwright"
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
  const theme = options.theme ?? process.env.PARITY_THEME ?? manifest.theme
  const uiStyle = options.uiStyle ?? process.env.PARITY_UI_STYLE ?? manifest.uiStyle ?? "classic"
  const uiScale = Number(
    options.uiScale ?? process.env.PARITY_UI_SCALE ?? manifest.uiScalePercent ?? 100
  )
  validateVariant(manifest, uiStyle, uiScale)
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
    const platform = selectedPlatform(manifest, options.platform)
    browser = await launchBrowser(platform)
    for (const screen of screens) {
      await captureScreen(
        browser,
        base,
        manifest,
        screen,
        outDir,
        platform,
        theme,
        uiStyle,
        uiScale
      )
    }
    const output = {
      version: 2,
      commit,
      theme,
      uiStyle,
      uiScalePercent: uiScale,
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

async function captureScreen(
  browser,
  base,
  manifest,
  screen,
  outDir,
  platform,
  theme,
  uiStyle,
  uiScale
) {
  const viewport =
    platform === "windows" ? (manifest.windowsViewport ?? manifest.viewport) : manifest.viewport
  const { width, height, deviceScaleFactor } = viewport
  const context = await browser.newContext({
    viewport: { width, height },
    deviceScaleFactor,
    colorScheme: theme === "light" ? "light" : "dark",
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
    url.searchParams.set("theme", theme)
    url.searchParams.set("scale", String(uiScale))
    url.searchParams.set("uiStyle", uiStyle)
    url.searchParams.set("actionsRequired", String((screen.actions ?? []).length > 0))
    url.searchParams.set("platform", platform)
    await page.goto(url.href, { waitUntil: "load" })
    if ((screen.actions ?? []).length > 0) {
      await runActions(page, screen.actions ?? [], screen.id)
      await page.evaluate(() => {
        window.__PARITY_ACTIONS_DONE__ = true
        window.dispatchEvent(new Event("parity-actions-done"))
      })
    }
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
    const missingEvents = await page.evaluate(() => window.__PARITY_MISSING_EVENTS__ ?? [])
    if (missingEvents.length > 0) {
      console.warn(`${screen.id}: missing event fixture(s): ${missingEvents.join(", ")}`)
    }
    const layout = await page.evaluate(collectLayout, screen.id)
    layout.missingEvents = missingEvents
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

async function runActions(page, actions, screenId) {
  for (const [index, action] of actions.entries()) {
    try {
      if (action.type === "waitFor") {
        await page.waitForSelector(targetSelector(action.target), { state: "visible" })
      } else if (action.type === "click") {
        await page.locator(targetSelector(action.target)).click()
      } else if (action.type === "hover") {
        await page.locator(targetSelector(action.target)).hover()
      } else if (action.type === "key") {
        const press = [...(action.modifiers ?? []).map((value) => `${value}+`), action.key].join("")
        if (action.target) await page.locator(targetSelector(action.target)).press(press)
        else await page.keyboard.press(press)
      } else if (action.type === "drag") {
        await page
          .locator(targetSelector(action.from))
          .dragTo(page.locator(targetSelector(action.to)))
      } else {
        throw new Error(`unknown action type ${action.type}`)
      }
    } catch (error) {
      throw new Error(
        `action ${index + 1} on ${screenId} failed: ${error instanceof Error ? error.message : String(error)}`
      )
    }
  }
}

function targetSelector(target) {
  if (!target || typeof target !== "string") throw new Error("action target is empty")
  return `[data-parity="${escapeSelectorValue(target)}"], [data-parity-action="${escapeSelectorValue(target)}"]`
}

function escapeSelectorValue(value) {
  return value.replaceAll("\\", "\\\\").replaceAll('"', '\\"')
}

function validateVariant(manifest, uiStyle, uiScale) {
  const styles = manifest.uiStyles ?? [manifest.uiStyle ?? "classic"]
  const scales = manifest.uiScalePercents ?? [manifest.uiScalePercent ?? 100]
  if (!styles.includes(uiStyle))
    throw new Error(`ui_style "${uiStyle}" is not enabled in parity/screens.json`)
  if (!scales.includes(uiScale))
    throw new Error(`ui_scale ${uiScale} is not enabled in parity/screens.json`)
}

async function launchBrowser(platform) {
  try {
    return platform === "windows"
      ? await chromium.launch({ channel: "msedge" })
      : await webkit.launch()
  } catch (error) {
    throw new Error(
      platform === "windows"
        ? `Could not start the installed Microsoft Edge channel. Confirm Edge is installed and Playwright 1.54.0 is available.`
        : `Could not start Playwright's WebKit. Install it with:\n` +
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
  const options = {
    out: undefined,
    commit: undefined,
    screens: [],
    platform: undefined,
    theme: undefined,
    uiStyle: undefined,
    uiScale: undefined,
  }
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
    } else if (argument === "--platform" && value) {
      options.platform = value
      index += 1
    } else if (argument === "--theme" && value) {
      options.theme = value
      index += 1
    } else if (argument === "--ui-style" && value) {
      options.uiStyle = value
      index += 1
    } else if (argument === "--ui-scale" && value) {
      options.uiScale = value
      index += 1
    } else {
      throw new Error(`Unknown or incomplete argument: ${argument}`)
    }
  }
  return options
}

function selectedPlatform(manifest, requested) {
  const platform = requested ?? process.env.PARITY_PLATFORM ?? "macos"
  if (platform !== "macos" && platform !== "windows") {
    throw new Error(`Unknown parity platform "${platform}"; expected macos or windows`)
  }
  if (platform !== "macos" && !manifest.platforms?.[platform]) {
    throw new Error(`parity/screens.json has no platform configuration for "${platform}"`)
  }
  return platform
}

main().catch((error) => {
  console.error(
    `reference capture failed: ${error instanceof Error ? error.message : String(error)}`
  )
  process.exitCode = 1
})
