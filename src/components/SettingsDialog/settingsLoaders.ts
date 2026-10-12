import { invoke } from "@tauri-apps/api/core"

import { normalizeScrollbackConfig } from "@/lib/scrollback"

const SECRET_STATUS_CACHE_MS = 30_000
const FONT_LOAD_TIMEOUT_MS = 5_000

export let cachedSystemFonts: string[] | null = null
let systemFontsPromise: Promise<string[]> | null = null
let lastSecretStatusRefreshAt = 0
let secretStatusPromise: Promise<unknown> | null = null

export function normalizeSettingsTab(tab: string) {
  return tab === "font" ? "terminal" : tab
}

export function normalizeScrollbackLines(value: number | undefined) {
  return normalizeScrollbackConfig(value)
}

export function getPerfNow() {
  return typeof performance === "undefined" ? 0 : performance.now()
}

export function logPerf(label: string, startTime: number, detail?: string) {
  if (!import.meta.env.DEV || typeof performance === "undefined") {
    return
  }

  const duration = Math.round(performance.now() - startTime)
  const suffix = detail ? ` ${detail}` : ""
  console.info(`[perf] ${label}: ${duration}ms${suffix}`)
}

export async function loadSystemFontsCached() {
  if (cachedSystemFonts !== null) {
    return cachedSystemFonts
  }

  if (systemFontsPromise) {
    return systemFontsPromise
  }

  const startTime = getPerfNow()
  const fontRequest = invoke<string[]>("list_fonts").then((fonts) => {
    cachedSystemFonts = fonts
    return fonts
  })

  systemFontsPromise = new Promise<string[]>((resolve, reject) => {
    const timeoutId = window.setTimeout(() => {
      reject(new Error("Timed out while loading system fonts"))
    }, FONT_LOAD_TIMEOUT_MS)

    fontRequest
      .then((fonts) => {
        window.clearTimeout(timeoutId)
        resolve(fonts)
      })
      .catch((error) => {
        window.clearTimeout(timeoutId)
        reject(error)
      })
  }).finally(() => {
    logPerf(
      "settings.fonts",
      startTime,
      cachedSystemFonts ? `count=${cachedSystemFonts.length}` : ""
    )
    systemFontsPromise = null
  })

  return systemFontsPromise
}

export function refreshSecretStatusCached(refreshSecretStatus: () => Promise<unknown>) {
  const now = Date.now()
  if (now - lastSecretStatusRefreshAt < SECRET_STATUS_CACHE_MS) {
    return
  }

  if (secretStatusPromise) {
    return
  }

  const startTime = getPerfNow()
  secretStatusPromise = refreshSecretStatus()
    .then(() => {
      lastSecretStatusRefreshAt = Date.now()
    })
    .catch(() => {})
    .finally(() => {
      logPerf("settings.secret_status", startTime)
      secretStatusPromise = null
    })
}
