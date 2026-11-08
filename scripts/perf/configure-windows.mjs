import fs from "node:fs/promises"
import path from "node:path"
import { createRequire } from "node:module"
const require = createRequire(import.meta.url)
const { chromium } = require("playwright")
const raw = path.resolve(import.meta.dirname, "../../target/perf")
const launch = JSON.parse(
  (await fs.readFile(path.join(raw, "launch.json"), "utf8")).replace(/^\uFEFF/, "")
)
let browser
for (let attempt = 0; attempt < 30; attempt++) {
  try {
    browser = await chromium.connectOverCDP(`http://127.0.0.1:${launch.port}`)
    break
  } catch (error) {
    if (attempt === 29) throw error
    await new Promise((resolve) => setTimeout(resolve, 1000))
  }
}
const page = browser.contexts()[0].pages()[0]
await page.waitForSelector(".xterm-helper-textarea")
await new Promise((resolve) => setTimeout(resolve, 2500))
const isolated = await page.evaluate(async () => ({
  profiles: await window.__TAURI__.core.invoke("list_profiles"),
  session: await window.__TAURI__.core.invoke("load_session"),
}))
if (
  isolated.profiles.length ||
  isolated.session.tabs.some((t) => t.connection?.type !== "terminal")
) {
  throw new Error("Unexpected connection; stop immediately without typing")
}
await fs.writeFile(path.join(raw, "isolation.json"), JSON.stringify(isolated, null, 2))
await page.evaluate(async () => {
  const config = await window.__TAURI__.core.invoke("load_config")
  Object.assign(config, {
    font_family: "Consolas",
    font_size: 14,
    scrollback_lines: 10000,
    auto_download_updates: false,
    update_check_frequency: "never",
  })
  await window.__TAURI__.core.invoke("save_config", { config })
  await window.__TAURI__.window
    .getCurrentWindow()
    .setSize(new window.__TAURI__.dpi.LogicalSize(1280, 800))
})
await page.reload()
await page.waitForSelector(".xterm-helper-textarea")
const conditions = await page.evaluate(async () => ({
  inner: await window.__TAURI__.window.getCurrentWindow().innerSize(),
  scale: await window.__TAURI__.window.getCurrentWindow().scaleFactor(),
  config: await window.__TAURI__.core.invoke("load_config"),
  visibility: document.visibilityState,
  focused: document.hasFocus(),
  screen: document.querySelector(".xterm-screen").getBoundingClientRect().toJSON(),
}))
await fs.writeFile(path.join(raw, "conditions.json"), JSON.stringify(conditions, null, 2))
await page.screenshot({ path: path.join(raw, "isolated-local.png") })
console.log(
  JSON.stringify({ isolation: isolated, inner: conditions.inner, scale: conditions.scale })
)
await browser.close()
