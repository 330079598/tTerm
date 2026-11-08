// P0-04 only: injects a small React DevTools hook into a Tauri development
// build, then drives the requested UI scenarios through CDP. It is intentionally
// kept out of the application bundle and must not be used as a product monitor.
import fs from "node:fs/promises"
import path from "node:path"
import { createRequire } from "node:module"

const require = createRequire(import.meta.url)
const { chromium } = require("playwright")

const repo = path.resolve(import.meta.dirname, "../..")
const output = path.join(repo, "target/p004-profile")
const port = Number(process.argv[2] ?? 9348)
const launchPath = process.argv[3] ?? path.join(repo, "target/p004-dev-launch.json")
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))

const hookSource = `(() => {
  const components = new Map();
  const commits = [];
  window.__P004_COMMITS__ = commits;
  const nameOf = (fiber) => {
    const value = fiber.elementType || fiber.type;
    if (typeof value === "string") return null;
    return value?.displayName || value?.name || (value?.render && (value.render.displayName || value.render.name)) || null;
  };
  const visit = (fiber, rows) => {
    for (let node = fiber; node; node = node.sibling) {
      const name = nameOf(node);
      if (name) {
        const alternate = node.alternate;
        const rendered = !alternate || node.actualDuration > 0 || node.memoizedProps !== alternate.memoizedProps || node.memoizedState !== alternate.memoizedState;
        const row = rows.get(name) || { name, renders: 0, actualDuration: 0, commits: 0 };
        if (rendered) row.renders += 1;
        row.actualDuration += Number(node.actualDuration || 0);
        if (rendered) row.commits += 1;
        rows.set(name, row);
      }
      if (node.child) visit(node.child, rows);
    }
  };
  const hook = {
    renderers: new Map(),
    _fiberRoots: new Map(),
    getFiberRoots(id) { if (!this._fiberRoots.has(id)) this._fiberRoots.set(id, new Set()); return this._fiberRoots.get(id); },
    checkDCE: () => false,
    supportsFiber: true,
    supportsFlight: false,
    onScheduleFiberRoot() {},
    inject(renderer) { const id = Math.max(0, ...Array.from(components.keys())) + 1; components.set(id, renderer); return id; },
    onCommitFiberRoot(id, root, priority) {
      try { hook.getFiberRoots(id).add(root); } catch (_) {}
      const rows = new Map();
      visit(root.current, rows);
      commits.push({ at: Date.now(), scenario: window.__P004_SCENARIO__ || "startup", rendererId: id, priority: String(priority ?? ""), components: Array.from(rows.values()) });
    },
    onCommitFiberUnmount() {},
    _p004: { commits }
  };
  window.__P004_COMMITS__ = commits;
  window.__REACT_DEVTOOLS_GLOBAL_HOOK__ = hook;
})();`

await fs.mkdir(output, { recursive: true })
const launch = JSON.parse(await fs.readFile(launchPath, "utf8"))
const browser = await chromium.connectOverCDP(`http://127.0.0.1:${launch.port ?? port}`)
const context = browser.contexts()[0]
const page = context.pages()[0]
const cdp = await context.newCDPSession(page)
await cdp.send("Page.enable")
await cdp.send("Page.addScriptToEvaluateOnNewDocument", { source: hookSource })
await page.reload({ waitUntil: "domcontentloaded" })
await page.waitForSelector(".xterm-helper-textarea", { timeout: 60000 })
await sleep(1500)

const cpuSample = async () => {
  const command =
    "(Get-Counter '\\Processor(_Total)\\% Processor Time').CounterSamples[0].CookedValue"
  const result = await new Promise((resolve) => {
    const child = require("node:child_process").execFile(
      "powershell.exe",
      ["-NoProfile", "-Command", command],
      (error, stdout) => resolve(error ? null : Number(stdout.trim()))
    )
    child.unref?.()
  })
  return Number.isFinite(result) ? result : null
}

async function setScenario(name) {
  await page.evaluate((value) => {
    window.__P004_SCENARIO__ = value
  }, name)
}

async function input(text) {
  await page.locator(".terminal-tab-shell.is-active .xterm-helper-textarea").focus()
  await cdp.send("Input.insertText", { text })
  for (const type of ["keyDown", "keyUp"]) {
    await cdp.send("Input.dispatchKeyEvent", {
      type,
      key: "Enter",
      code: "Enter",
      windowsVirtualKeyCode: 13,
    })
  }
}

async function action(name, fn, wait = 1200) {
  await setScenario(name)
  const started = Date.now()
  let error = null
  try {
    await fn()
  } catch (cause) {
    error = String(cause)
  }
  await sleep(wait)
  return { name, started, ended: Date.now(), error }
}

const actions = []
const cpu = []
cpu.push({ at: Date.now(), value: await cpuSample() })

actions.push(
  await action(
    "continuous-output",
    async () => {
      await input(
        '$i=0; 1..1000 | ForEach-Object { $i++; "P004|$([DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds())|$i" }; Write-Output P004_DONE'
      )
    },
    3500
  )
)
cpu.push({ at: Date.now(), value: await cpuSample() })

actions.push(
  await action(
    "switch-tab",
    async () => {
      await page.keyboard.press("Control+Shift+t")
      await sleep(900)
      await page.keyboard.press("Control+Tab")
      await page.keyboard.press("Control+Tab")
    },
    1000
  )
)
cpu.push({ at: Date.now(), value: await cpuSample() })

actions.push(
  await action(
    "split-drag",
    async () => {
      const handle = page
        .locator('[data-panel-resize-handle-id], [class*="resize-handle"], [role="separator"]')
        .first()
      if ((await handle.count()) === 0) throw new Error("no visible split resize handle")
      const box = await handle.boundingBox()
      if (!box) throw new Error("split resize handle has no box")
      await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2)
      await page.mouse.down()
      await page.mouse.move(box.x + box.width / 2 + 80, box.y + box.height / 2 + 20, { steps: 8 })
      await page.mouse.up()
    },
    1000
  )
)
cpu.push({ at: Date.now(), value: await cpuSample() })

actions.push(
  await action(
    "settings",
    async () => {
      await page
        .locator(
          '[data-state="open"] [aria-label="Close"], [data-state="open"] button[aria-label="Close"]'
        )
        .first()
        .click({ timeout: 3000 })
        .catch(() => {})
      await page.getByRole("button", { name: "Settings" }).click({ force: true })
    },
    1200
  )
)
cpu.push({ at: Date.now(), value: await cpuSample() })

// SFTP is deliberately not opened: without an SSH connection it is not the
// real drawer scenario, and this run must never connect to a host.
await setScenario("sftp-drawer-artificial-skipped")

const react = await page.evaluate(() => ({
  hook: Boolean(window.__REACT_DEVTOOLS_GLOBAL_HOOK__),
  commits: window.__P004_COMMITS__ ?? window.__REACT_DEVTOOLS_GLOBAL_HOOK__?._p004?.commits ?? [],
}))

async function timestamped(lines, name) {
  await setScenario(name)
  await page.keyboard.press("Escape").catch(() => {})
  await page.locator(".terminal-tab-shell.is-active .xterm-helper-textarea").focus()
  await page
    .locator(
      '[data-state="open"] [aria-label="Close"], [data-state="open"] button[aria-label="Close"]'
    )
    .first()
    .click({ timeout: 3000 })
    .catch(() => {})
  await page.locator(".terminal-tab-shell.is-active .xterm-helper-textarea").focus()
  const command = `$i=0; 1..${lines} | ForEach-Object { $i++; Write-Output ('P004LAT|' + [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() + '|' + $i) }`
  const started = Date.now()
  await input(command)
  const samples = []
  const deadline = Date.now() + Math.max(8000, lines * 25)
  let lastText = ""
  while (Date.now() < deadline && samples.length < lines) {
    const text = await page
      .locator(".terminal-tab-shell.is-active .xterm-rows")
      .innerText()
      .catch(() => "")
    if (text !== lastText) {
      lastText = text
      for (const match of text.matchAll(/P004LAT\|(\d{13})\|(\d+)/g)) {
        const stamp = Number(match[1])
        if (!samples.some((item) => item.sequence === Number(match[2])))
          samples.push({ sequence: Number(match[2]), latencyMs: Date.now() - stamp })
      }
    }
    await sleep(25)
  }
  return { name, lines, started, finished: Date.now(), parsedSamples: samples }
}

const latency = [
  await timestamped(20, "latency-small"),
  await timestamped(2000, "latency-sustained"),
]
const result = {
  generatedAt: new Date().toISOString(),
  port: launch.port ?? port,
  cpu,
  actions,
  react,
  latency,
  hook: "Page.addScriptToEvaluateOnNewDocument; React DevTools supportsFiber hook",
}
await fs.writeFile(path.join(output, "raw.json"), JSON.stringify(result, null, 2))
console.log(
  JSON.stringify(
    {
      actions,
      commits: react.commits.length,
      latency: latency.map(({ name, lines, parsedSamples }) => ({
        name,
        lines,
        parsed: parsedSamples.length,
      })),
    },
    null,
    2
  )
)
await browser.close()
