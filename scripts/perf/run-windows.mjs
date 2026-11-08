import fs from "node:fs/promises"
import path from "node:path"
import { spawn } from "node:child_process"
import { gzipSync } from "node:zlib"
import { createRequire } from "node:module"

const require = createRequire(import.meta.url)
const { chromium } = require("playwright")
const repo = path.resolve(import.meta.dirname, "../..")
const raw = path.join(repo, "target/perf")
const launch = JSON.parse(
  (await fs.readFile(path.join(raw, "launch.json"), "utf8")).replace(/^\uFEFF/, "")
)
const [run, ...loads] = process.argv.slice(2)
if (!["1", "2"].includes(run) || !loads.length)
  throw new Error("Usage: node run-windows.mjs 1|2 LOAD...")
const browser = await chromium.connectOverCDP(`http://127.0.0.1:${launch.port}`)
const page = browser.contexts()[0].pages()[0]
const cdp = await page.context().newCDPSession(page)
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
const session = () => page.evaluate(() => window.__TAURI__.core.invoke("load_session"))
async function guard() {
  const profiles = await page.evaluate(() => window.__TAURI__.core.invoke("list_profiles"))
  const state = await session()
  if (profiles.length || state.tabs.some((t) => t.connection?.type !== "terminal")) {
    throw new Error("Unexpected connection: stop immediately without terminal input")
  }
  return state
}
async function input(text) {
  await guard()
  await page.locator(".terminal-tab-shell.is-active .xterm-helper-textarea").focus()
  await cdp.send("Input.insertText", { text })
  await cdp.send("Input.dispatchKeyEvent", {
    type: "keyDown",
    key: "Enter",
    code: "Enter",
    windowsVirtualKeyCode: 13,
  })
  await cdp.send("Input.dispatchKeyEvent", {
    type: "keyUp",
    key: "Enter",
    code: "Enter",
    windowsVirtualKeyCode: 13,
  })
}
for (const load of loads) {
  await guard()
  const idle = load.match(/^idle-(1|10|30)$/)
  if (idle) {
    let state = await session()
    if (state.tabs.length > Number(idle[1]))
      throw new Error("Restart isolated app before reducing idle count")
    while (state.tabs.length < Number(idle[1])) {
      await page.keyboard.press("Control+Shift+t")
      await page.getByRole("option").filter({ hasText: "Auto (recommended)" }).first().click()
      await sleep(2500)
      const next = await session()
      if (next.tabs.length !== state.tabs.length + 1)
        throw new Error("New local tab was not created")
      state = next
    }
    await sleep(15000)
  } else if ((await session()).tabs.length !== 1) {
    throw new Error("Output workloads require exactly one local tab")
  }
  const before = await page.evaluate(async () => ({
    session: await window.__TAURI__.core.invoke("load_session"),
    visibility: document.visibilityState,
    focused: document.hasFocus(),
    inner: await window.__TAURI__.window.getCurrentWindow().innerSize(),
    scale: await window.__TAURI__.window.getCurrentWindow().scaleFactor(),
  }))
  if (before.visibility !== "visible" || !before.focused)
    throw new Error("Window must be visible and focused")
  await fs.writeFile(
    path.join(raw, `run${run}-${load}-session.json`),
    JSON.stringify(before, null, 2)
  )
  const collector = spawn(
    "C:/Program Files/PowerShell/7/pwsh.exe",
    [
      "-NoProfile",
      "-File",
      path.join(repo, "scripts/perf/collect-windows.ps1"),
      "-RootPid",
      String(launch.pid),
      "-Workload",
      load,
      "-Seconds",
      "40",
      "-Output",
      path.join(raw, `run${run}-${load}.json`),
    ],
    { cwd: repo, windowsHide: true, stdio: ["ignore", "pipe", "pipe"] }
  )
  let collectorLog = ""
  collector.stdout.on("data", (b) => {
    collectorLog += b.toString()
  })
  collector.stderr.on("data", (b) => {
    collectorLog += b.toString()
  })
  const completed = new Promise((resolve, reject) => {
    collector.on("error", reject)
    collector.on("exit", (code) => (code === 0 ? resolve() : reject(new Error(collectorLog))))
  })
  await cdp.send("Tracing.start", {
    categories: "devtools.timeline,disabled-by-default-devtools.timeline.frame,benchmark",
    transferMode: "ReturnAsStream",
  })
  if (!idle) {
    const wslRepo = "/mnt/d/code/rust/tTerm-P0-01"
    await input(
      `wsl.exe -d Ubuntu -- bash ${wslRepo}/scripts/perf/workload.sh ${load} ${wslRepo}/target/perf/load-run${run}`
    )
  }
  const inProgress = sleep(10000).then(() =>
    page.screenshot({ path: path.join(raw, `run${run}-${load}-active.png`) })
  )
  await completed
  await inProgress
  const tracingDone = new Promise((resolve) => cdp.once("Tracing.tracingComplete", resolve))
  await cdp.send("Tracing.end")
  const { stream } = await tracingDone
  const chunks = []
  while (true) {
    const part = await cdp.send("IO.read", { handle: stream })
    chunks.push(Buffer.from(part.data, part.base64Encoded ? "base64" : "utf8"))
    if (part.eof) break
  }
  await cdp.send("IO.close", { handle: stream })
  await fs.writeFile(
    path.join(raw, `run${run}-${load}.trace.json.gz`),
    gzipSync(Buffer.concat(chunks))
  )
  await fs.writeFile(path.join(raw, `run${run}-${load}.collector.txt`), collectorLog)
  await page.screenshot({ path: path.join(raw, `run${run}-${load}.png`) })
  console.log(`run${run} ${load} complete; tabs=${(await session()).tabs.length}`)
}
await browser.close()
