// Runs the real TypeScript modules of the P1-11c set in Node and prints the
// values the Rust port has to reproduce. Every case in the matching
// `*.test.ts` files is transcribed here verbatim, so a failure means either the
// transcription or the module changed.
//
// Usage:
//   P11C_ROOT=<repository> P11C_PLATFORM=macos P11C_VERSION=0.8.1 \
//     node scripts/migration/P1-11c-oracle.mjs > out.json
//
// The loader in this file maps the `@/` alias to `src/`, stubs the two Tauri
// packages the config modules import at runtime, turns JSON imports into ES
// modules and substitutes `import.meta.env.PACKAGE_VERSION`, which Vite fills
// in at build time. Nothing under `src/` is modified.

import assert from "node:assert/strict"
import { readFile } from "node:fs/promises"
import { register } from "node:module"
import { pathToFileURL } from "node:url"

const ROOT = process.env.P11C_ROOT
const PLATFORM = process.env.P11C_PLATFORM ?? "macos"
const VERSION = process.env.P11C_VERSION ?? "0.8.1"

if (!ROOT) {
  console.error("P11C_ROOT must name the repository root")
  process.exit(2)
}

const loaderSource = `
import { access, readFile } from "node:fs/promises"
import { pathToFileURL } from "node:url"

const ROOT = ${JSON.stringify(ROOT)}
const PLATFORM = ${JSON.stringify(PLATFORM)}
const VERSION = ${JSON.stringify(VERSION)}

const STUBS = {
  "p11c:tauri-os": "export const platform = () => " + JSON.stringify(PLATFORM),
  "p11c:xterm": "export class Terminal { constructor() {} }",
  // clsx and tailwind-merge only serve cn(); nothing in this set calls it.
  "p11c:clsx": "export const clsx = (...args) => args.flat().filter(Boolean).join(' ')",
  "p11c:tailwind-merge": "export const twMerge = (value) => value",
}

export async function resolve(specifier, context, next) {
  if (specifier === "@tauri-apps/plugin-os") {
    return { url: "p11c:tauri-os", shortCircuit: true }
  }
  if (specifier === "@xterm/xterm") {
    return { url: "p11c:xterm", shortCircuit: true }
  }
  if (specifier === "clsx") {
    return { url: "p11c:clsx", shortCircuit: true }
  }
  if (specifier === "tailwind-merge") {
    return { url: "p11c:tailwind-merge", shortCircuit: true }
  }
  if (specifier.startsWith("@/")) {
    const base = ROOT + "/src/" + specifier.slice(2)
    for (const candidate of [base, base + ".ts", base + ".tsx", base + "/index.ts", base + ".json"]) {
      try {
        await access(candidate)
        return { url: pathToFileURL(candidate).href, shortCircuit: true }
      } catch {}
    }
  }
  return next(specifier, context)
}

export async function load(url, context, next) {
  if (STUBS[url]) {
    return { format: "module", source: STUBS[url], shortCircuit: true }
  }
  if (url.endsWith(".json")) {
    const text = await readFile(new URL(url), "utf8")
    return { format: "module", source: "export default " + text, shortCircuit: true }
  }
  if (url.endsWith(".ts") || url.endsWith(".tsx")) {
    let source = await readFile(new URL(url), "utf8")
    if (source.includes("import.meta.env.PACKAGE_VERSION")) {
      source = source.replaceAll("import.meta.env.PACKAGE_VERSION", JSON.stringify(VERSION))
    }
    return { format: "module-typescript", source, shortCircuit: true }
  }
  return next(url, context)
}
`

register(`data:text/javascript,${encodeURIComponent(loaderSource)}`)

const load = (path) => import(pathToFileURL(`${ROOT}/src/${path}`).href)

const cases = []
const values = {}

/** Drops `undefined` members, the way the tests' `toEqual` ignores them. */
function clean(value) {
  if (Array.isArray(value)) return value.map(clean)
  if (value && typeof value === "object") {
    const out = {}
    for (const [key, member] of Object.entries(value)) {
      if (member !== undefined) out[key] = clean(member)
    }
    return out
  }
  return value
}

function subset(value, keys) {
  return Object.fromEntries(keys.map((key) => [key, value[key]]))
}

function check(id, input, expected, actual) {
  const cleanedActual = clean(actual)
  try {
    assert.deepStrictEqual(cleanedActual, clean(expected))
  } catch (error) {
    cases.push({ id, input, expected: clean(expected), actual: cleanedActual, pass: false })
    console.error(`FAILED ${id}\n${error.message}`)
    return
  }
  cases.push({ id, input, expected: clean(expected), actual: cleanedActual, pass: true })
}

/** Whether the version has an alpha, beta, rc or dev part, as Rust decides it. */
function prereleaseVersion(version) {
  return version
    .split("-")
    .slice(1)
    .some((part) =>
      ["alpha", "beta", "rc", "dev"].some(
        (tag) => part === tag || part.startsWith(`${tag}.`)
      )
    )
}

function record(id, value) {
  values[id] = clean(value)
  return value
}

/** A small DOM and storage shim for `uiStyle` and `startup`. */
function installBrowserShim() {
  const classes = new Set()
  const stored = new Map()
  const listeners = new Map()

  const documentElement = {
    classList: {
      add(name) {
        classes.add(name)
      },
      remove(name) {
        classes.delete(name)
      },
      contains(name) {
        return classes.has(name)
      },
      toggle(name, force) {
        const on = force === undefined ? !classes.has(name) : force
        if (on) classes.add(name)
        else classes.delete(name)
        return on
      },
    },
    style: { setProperty() {} },
  }

  Object.defineProperty(globalThis, "document", {
    configurable: true,
    value: { documentElement, createElement: () => ({}) },
  })
  Object.defineProperty(globalThis, "localStorage", {
    configurable: true,
    value: {
      getItem: (key) => (stored.has(key) ? stored.get(key) : null),
      setItem: (key, value) => stored.set(key, String(value)),
      removeItem: (key) => stored.delete(key),
      clear: () => stored.clear(),
    },
  })
  Object.defineProperty(globalThis, "navigator", {
    configurable: true,
    value: {
      languages: ["en-US"],
      language: "en-US",
      platform: "MacIntel",
      userAgent: "node",
    },
  })
  Object.defineProperty(globalThis, "CustomEvent", {
    configurable: true,
    value: class {
      constructor(type) {
        this.type = type
      }
    },
  })
  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: {
      dispatchEvent(event) {
        for (const listener of [...(listeners.get(event.type) ?? [])]) listener(event)
        return true
      },
      addEventListener(type, listener) {
        listeners.set(type, [...(listeners.get(type) ?? []), listener])
      },
      removeEventListener(type, listener) {
        listeners.set(type, (listeners.get(type) ?? []).filter((other) => other !== listener))
      },
    },
  })

  return {
    reset() {
      classes.clear()
      stored.clear()
      listeners.clear()
    },
    classes,
    stored,
  }
}

const shim = installBrowserShim()

// ---------------------------------------------------------------- tabSettings
{
  const { DEFAULT_TAB_SETTINGS, normalizeTabSettings, INACTIVE_PANE_DIM_OPTIONS } = await load(
    "lib/tabSettings.ts"
  )
  record("tabSettings.defaults", DEFAULT_TAB_SETTINGS)
  record("tabSettings.inactivePaneDimOptions", INACTIVE_PANE_DIM_OPTIONS)
  const tab = (config) => normalizeTabSettings(config)

  check(
    "tabSettings.test › normalizeTabSettings › knows only the two width modes",
    { tab_width_mode: ["standard", "wide"] },
    ["standard", "adaptive"],
    [tab({ tab_width_mode: "standard" }).tab_width_mode, tab({ tab_width_mode: "wide" }).tab_width_mode]
  )
  const width = (value) => tab({ tab_standard_width: value }).tab_standard_width
  check(
    "tabSettings.test › normalizeTabSettings › keeps the standard width in range",
    [150.4, 20, 999],
    [150, 80, 300],
    [width(150.4), width(20), width(999)]
  )
  check(
    "tabSettings.test › normalizeTabSettings › shows the hover tooltip only when turned on",
    [true, "yes"],
    [true, false],
    [tab({ tab_hover_tooltip: true }).tab_hover_tooltip, tab({ tab_hover_tooltip: "yes" }).tab_hover_tooltip]
  )
  check("tabSettings.test › normalizeTabSettings › fills in the defaults", {}, DEFAULT_TAB_SETTINGS, tab({}))
  const dim = (value) => tab({ panes: { dim_inactive_percent: value } }).panes.dim_inactive_percent
  check(
    "tabSettings.test › pane settings › shows headers unless turned off, and keeps the fade in range",
    { show_headers: [false, "no"], dim: [30.4, -5, 200, "30"] },
    { show_headers: [false, true], dim: [30, 0, 60, 0] },
    {
      show_headers: [
        tab({ panes: { show_headers: false } }).panes.show_headers,
        tab({ panes: { show_headers: "no" } }).panes.show_headers,
      ],
      dim: [dim(30.4), dim(-5), dim(200), dim("30")],
    }
  )
  check(
    "tabSettings.test › pane settings › drops saved layouts that are broken",
    { panes: { templates: [{ id: "a", name: "x", root: { type: "pane" } }] } },
    [],
    tab({ panes: { templates: [{ id: "a", name: "x", root: { type: "pane" } }] } }).panes.templates
  )
}

// ---------------------------------------------------------------- terminalFont
{
  const { defaultTerminalFontFamily, firstInstalledFont } = await load("lib/terminalFont.ts")
  record("terminalFont.families", {
    macos: defaultTerminalFontFamily("macos"),
    windows: defaultTerminalFontFamily("windows"),
    linux: defaultTerminalFontFamily("linux"),
    freebsd: defaultTerminalFontFamily("freebsd"),
    detected: defaultTerminalFontFamily(),
  })
  check(
    "terminalFont.test › defaultTerminalFontFamily › puts a Nerd Font before each system's monospace font",
    ["macos", "windows", "linux"],
    [true, true, true],
    [
      /^"JetBrains Mono Nerd Font", .*Menlo/.test(defaultTerminalFontFamily("macos")),
      /^"JetBrains Mono Nerd Font", .*Consolas/.test(defaultTerminalFontFamily("windows")),
      /^"JetBrains Mono Nerd Font", .*DejaVu/.test(defaultTerminalFontFamily("linux")),
    ]
  )
  check(
    "terminalFont.test › defaultTerminalFontFamily › gives an unknown platform Linux's fonts",
    "freebsd",
    true,
    defaultTerminalFontFamily("freebsd") === defaultTerminalFontFamily("linux")
  )
  const chain = '"JetBrains Mono Nerd Font", "JetBrains Mono", Menlo, monospace'
  check(
    "terminalFont.test › firstInstalledFont › names the first font of the chain the system has",
    { fonts: [["Menlo", "jetbrains mono"], ["Menlo"]] },
    ["JetBrains Mono", "Menlo"],
    [firstInstalledFont(chain, ["Menlo", "jetbrains mono"]), firstInstalledFont(chain, ["Menlo"])]
  )
  check(
    "terminalFont.test › firstInstalledFont › keeps the chain when none of it is installed",
    [],
    chain,
    firstInstalledFont(chain, [])
  )
  // The chain entries as the port has to read them, one quote stripped.
  record("terminalFont.entries", {
    chain,
    entries: chain.split(",").map((name) => name.trim().replace(/^["']|["']$/g, "")),
    lookup: ["Menlo", "jetbrains mono"].map((name) => name.toLowerCase()),
  })
}

// ---------------------------------------------------------------- scrollback
{
  const {
    DEFAULT_SCROLLBACK_LINES,
    MAX_EXPLICIT_SCROLLBACK_LINES,
    UNLIMITED_SCROLLBACK_BUFFER,
    UNLIMITED_SCROLLBACK_INITIAL,
    UNLIMITED_SCROLLBACK_SENTINEL,
    countLineFeeds,
    grownScrollbackCapacity,
    isUnlimitedScrollback,
    normalizeScrollbackConfig,
    resolveScrollbackLines,
    startingScrollbackLines,
  } = await load("lib/scrollback.ts")

  record("scrollback.constants", {
    DEFAULT_SCROLLBACK_LINES,
    MAX_EXPLICIT_SCROLLBACK_LINES,
    UNLIMITED_SCROLLBACK_BUFFER,
    UNLIMITED_SCROLLBACK_INITIAL,
    UNLIMITED_SCROLLBACK_SENTINEL,
  })
  check(
    "scrollback.test › startingScrollbackLines › starts unlimited scrollback small instead of at the full buffer",
    0,
    UNLIMITED_SCROLLBACK_INITIAL,
    startingScrollbackLines(0)
  )
  check(
    "scrollback.test › startingScrollbackLines › passes explicit limits through",
    [5000, "undefined"],
    [5000, 10_000],
    [startingScrollbackLines(5000), startingScrollbackLines(undefined)]
  )
  check(
    "scrollback.test › countLineFeeds › counts LF, VT and FF but not CR",
    ["a\r\nb\nc\x0bd\x0ce\r", ""],
    [4, 0],
    [countLineFeeds("a\r\nb\nc\x0bd\x0ce\r"), countLineFeeds("")]
  )
  check(
    "scrollback.test › grownScrollbackCapacity › keeps the capacity while half of it is still free",
    [10_000, 5000],
    null,
    grownScrollbackCapacity(10_000, 5000)
  )
  check(
    "scrollback.test › grownScrollbackCapacity › doubles until the lines fit twice over",
    [
      [10_000, 5001],
      [10_000, 30_000],
    ],
    [20_000, 80_000],
    [grownScrollbackCapacity(10_000, 5001), grownScrollbackCapacity(10_000, 30_000)]
  )
  check(
    "scrollback.test › grownScrollbackCapacity › never grows past the unlimited buffer",
    [10_000, UNLIMITED_SCROLLBACK_BUFFER],
    UNLIMITED_SCROLLBACK_BUFFER,
    grownScrollbackCapacity(10_000, UNLIMITED_SCROLLBACK_BUFFER)
  )
  record("scrollback.normalize", {
    inputs: [undefined, 0, 1, 5000, 10_000_000, 10_000_001, -3, 0.4, 0.6],
    stored: [undefined, 0, 1, 5000, 10_000_000, 10_000_001, -3, 0.4, 0.6].map(normalizeScrollbackConfig),
    resolved: [undefined, 0].map(resolveScrollbackLines),
    starting: [undefined, 0, 5000].map(startingScrollbackLines),
    unlimited: [undefined, 0, 1].map(isUnlimitedScrollback),
  })
}

// ---------------------------------------------------------------- uiStyle
{
  const { applyUiStyle, preloadUiStyle } = await load("lib/uiStyle.ts")
  const root = () => globalThis.document.documentElement

  shim.reset()
  applyUiStyle("islands")
  const islands = root().classList.contains("ui-islands")
  applyUiStyle("classic")
  const classic = root().classList.contains("ui-islands")
  check("uiStyle.test › ui style › marks the page for islands and clears the mark for classic", "islands then classic", [true, false], [islands, classic])

  shim.reset()
  applyUiStyle("islands")
  root().classList.remove("ui-islands")
  preloadUiStyle()
  check("uiStyle.test › ui style › starts the next launch in the style last applied", "islands", true, root().classList.contains("ui-islands"))

  shim.reset()
  applyUiStyle("classic")
  preloadUiStyle()
  check("uiStyle.test › ui style › starts the next launch classic when classic was last applied", "classic", false, root().classList.contains("ui-islands"))

  shim.reset()
  preloadUiStyle()
  check("uiStyle.test › ui style › starts in islands when nothing was remembered", "nothing", true, root().classList.contains("ui-islands"))
  record("uiStyle.storageKey", ["tterm-ui-style", "ui-islands"])
  shim.reset()
}

// ---------------------------------------------------------------- spinnerStyle
{
  const { getSpinnerStyle, setSpinnerStyle, subscribeSpinnerStyle } = await load("lib/spinnerStyle.ts")
  const calls = []
  check("spinnerStyle.test › spinnerStyle › starts with the default working style", "initial", "ring", getSpinnerStyle())
  const unsubscribe = subscribeSpinnerStyle(() => calls.push(getSpinnerStyle()))
  setSpinnerStyle("comet")
  setSpinnerStyle("comet")
  const afterChange = getSpinnerStyle()
  unsubscribe()
  setSpinnerStyle("dots")
  check(
    "spinnerStyle.test › spinnerStyle › tells subscribers about a change, and only a change",
    ["comet", "comet", "dots"],
    { style: "comet", calls: 1 },
    { style: afterChange, calls: calls.length }
  )
  setSpinnerStyle("ring")
  record("spinnerStyle.calls", calls)
}

// ---------------------------------------------------------------- shellCommands
{
  const { splitShellCommands } = await load("lib/shellCommands.ts")
  const split = (...lines) => splitShellCommands(lines.join("\n"))

  check(
    "shellCommands.test › splitShellCommands › keeps each plain line apart and drops blank ones",
    ["cd /tmp", "", "  ", "ls -la # list", ""],
    ["cd /tmp", "ls -la # list"],
    split("cd /tmp", "", "  ", "ls -la # list", "")
  )
  check(
    "shellCommands.test › splitShellCommands › keeps a line ending in a backslash with the next",
    ["docker run \\", "  -it alpine", "echo done"],
    ["docker run \\\n  -it alpine", "echo done"],
    split("docker run \\", "  -it alpine", "echo done")
  )
  check("shellCommands.test › splitShellCommands › an escaped backslash ends the line", ["echo a\\\\", "pwd"], ["echo a\\\\", "pwd"], split("echo a\\\\", "pwd"))
  check(
    "shellCommands.test › splitShellCommands › keeps an open quote with the lines up to its end",
    ["echo 'one", "", "two' \"x", 'y"', "pwd"],
    ["echo 'one\n\ntwo' \"x\ny\"", "pwd"],
    split("echo 'one", "", "two' \"x", 'y"', "pwd")
  )
  check("shellCommands.test › splitShellCommands › a quote inside a quote", ["echo \"it's\"", "pwd"], ['echo "it\'s"', "pwd"], split('echo "it\'s"', "pwd"))
  check(
    "shellCommands.test › splitShellCommands › keeps a here-document with its body, up to the line that ends it",
    ["cat <<'EOF' > a.txt", "if then", "", "EOF", "pwd"],
    ["cat <<'EOF' > a.txt\nif then\n\nEOF", "pwd"],
    split("cat <<'EOF' > a.txt", "if then", "", "EOF", "pwd")
  )
  check(
    "shellCommands.test › splitShellCommands › a here-document that strips tabs",
    ["cat <<-END", "\tx", "\tEND", "pwd"],
    ["cat <<-END\n\tx\n\tEND", "pwd"],
    split("cat <<-END", "\tx", "\tEND", "pwd")
  )
  check("shellCommands.test › splitShellCommands › a here-string", ["grep x <<< 'y'", "pwd"], ["grep x <<< 'y'", "pwd"], split("grep x <<< 'y'", "pwd"))
  check(
    "shellCommands.test › splitShellCommands › keeps a block together, however its lines are laid out",
    ["for f in *.log", "do", "  gzip $f", "done", "ls"],
    ["for f in *.log\ndo\n  gzip $f\ndone", "ls"],
    split("for f in *.log", "do", "  gzip $f", "done", "ls")
  )
  check(
    "shellCommands.test › splitShellCommands › if/elif blocks",
    ["if [ -f a ]; then", "  echo a", "elif [ -f b ]; then", "  echo b", "fi", "ls"],
    ["if [ -f a ]; then\n  echo a\nelif [ -f b ]; then\n  echo b\nfi", "ls"],
    split("if [ -f a ]; then", "  echo a", "elif [ -f b ]; then", "  echo b", "fi", "ls")
  )
  check(
    "shellCommands.test › splitShellCommands › a one-line while block",
    ["while read l; do echo $l; done < f", "ls"],
    ["while read l; do echo $l; done < f", "ls"],
    split("while read l; do echo $l; done < f", "ls")
  )
  check(
    "shellCommands.test › splitShellCommands › a case block",
    ['case "$1" in', "  a) echo a ;;", "  *) echo other ;;", "esac", "ls"],
    ['case "$1" in\n  a) echo a ;;\n  *) echo other ;;\nesac', "ls"],
    split('case "$1" in', "  a) echo a ;;", "  *) echo other ;;", "esac", "ls")
  )
  check(
    "shellCommands.test › splitShellCommands › braces and parentheses that span lines",
    ["greet() {", '  echo "hi ${1}"', "}", "greet"],
    ['greet() {\n  echo "hi ${1}"\n}', "greet"],
    split("greet() {", '  echo "hi ${1}"', "}", "greet")
  )
  check(
    "shellCommands.test › splitShellCommands › a parenthesised block",
    ["(cd /tmp &&", "  ls)", "pwd"],
    ["(cd /tmp &&\n  ls)", "pwd"],
    split("(cd /tmp &&", "  ls)", "pwd")
  )
  check(
    "shellCommands.test › splitShellCommands › a PowerShell block",
    ["if ($x) {", "  'yes'", "}", "Get-Date"],
    ["if ($x) {\n  'yes'\n}", "Get-Date"],
    split("if ($x) {", "  'yes'", "}", "Get-Date")
  )
  check(
    "shellCommands.test › splitShellCommands › a fish block",
    ["for f in *.txt", "  echo $f", "end", "ls"],
    ["for f in *.txt\n  echo $f\nend", "ls"],
    split("for f in *.txt", "  echo $f", "end", "ls")
  )
  check(
    "shellCommands.test › splitShellCommands › takes no keyword from an argument, a quote or a comment",
    ["echo if for done", "grep 'while' f", "ls # if", "pwd"],
    ["echo if for done", "grep 'while' f", "ls # if", "pwd"],
    split("echo if for done", "grep 'while' f", "ls # if", "pwd")
  )
  check(
    "shellCommands.test › splitShellCommands › still sends a command left open at the end",
    ["ls", "echo 'oops"],
    ["ls", "echo 'oops"],
    split("ls", "echo 'oops")
  )
  record("shellCommands.extra", {
    crlf: splitShellCommands("cd /tmp\r\nls\r"),
    gluedKeyword: splitShellCommands('if"x"', "then"),
    heredocX: splitShellCommands("cat <<X", "a", "X", "ls"),
  })
}

// ---------------------------------------------------------------- sshShellIntegration
{
  const {
    announcesShellIntegrationFallback,
    shellIntegrationChoice,
    shellIntegrationPayload,
    sshShellIntegrationEnabled,
  } = await load("lib/sshShellIntegration.ts")

  check(
    "sshShellIntegration.test › shellIntegrationChoice › round-trips through the stored value",
    [null, true, false],
    [null, true, false],
    [undefined, true, false].map((value) => cleanValue(shellIntegrationPayload(shellIntegrationChoice(value))))
  )
  function cleanValue(value) {
    return value === undefined ? null : value
  }
  check(
    "sshShellIntegration.test › sshShellIntegrationEnabled › follows the setting unless the connection chose",
    [
      [null, true],
      [null, false],
      [false, true],
      [true, false],
    ],
    [true, false, false, true],
    [
      sshShellIntegrationEnabled(undefined, true),
      sshShellIntegrationEnabled(undefined, false),
      sshShellIntegrationEnabled(false, true),
      sshShellIntegrationEnabled(true, false),
    ]
  )
  const quietReasons = [{ code: "shell", shell: "tcsh" }, { code: "noReply" }, { code: "writeFailed" }, { code: "path" }]
  check(
    "sshShellIntegration.test › announcesShellIntegrationFallback › stays quiet about the host's own answers unless the connection turned it on",
    quietReasons,
    quietReasons.map(() => [false, true]),
    quietReasons.map((reason) => [
      announcesShellIntegrationFallback(undefined, reason),
      announcesShellIntegrationFallback(true, reason),
    ])
  )
  check(
    "sshShellIntegration.test › announcesShellIntegrationFallback › tells about failures a reconnect may not meet again",
    ["rejected", "timedOut", "failed"],
    [true, true, true],
    ["rejected", "timedOut", "failed"].map((code) => announcesShellIntegrationFallback(undefined, { code }))
  )
  record("sshShellIntegration.choice", {
    missing: shellIntegrationChoice(undefined),
    on: shellIntegrationChoice(true),
    off: shellIntegrationChoice(false),
    payload: ["default", "on", "off"].map((choice) => cleanValue(shellIntegrationPayload(choice))),
  })
}

// ---------------------------------------------------------------- remoteShellIntegration
{
  const { FISH_SHELL_INTEGRATION, POSIX_SHELL_INTEGRATION } = await load("lib/remoteShellIntegration.ts")
  check(
    "remoteShellIntegration.test › scripts › report the directory with `%` escaped, as the shell expands it",
    "counts",
    [2, 2, true],
    [
      POSIX_SHELL_INTEGRATION.split("file://%s").length - 1,
      POSIX_SHELL_INTEGRATION.split('"${PWD//\\%/%25}"').length - 1,
      FISH_SHELL_INTEGRATION.includes("(string replace --all '%' '%25' -- $PWD)"),
    ]
  )
  check(
    "remoteShellIntegration.test › scripts › name the host, so a shell on a host reached with ssh is told apart",
    "counts",
    [2, true, true, true, true],
    [
      POSIX_SHELL_INTEGRATION.split("file://%s%s").length - 1,
      POSIX_SHELL_INTEGRATION.includes('"$?" "$HOST" "${PWD'),
      POSIX_SHELL_INTEGRATION.includes('"$s" "$HOSTNAME" "${PWD'),
      FISH_SHELL_INTEGRATION.includes("file://%s%s"),
      FISH_SHELL_INTEGRATION.includes("' $hostname (string replace"),
    ]
  )
  record("remoteShellIntegration.scripts", { posix: POSIX_SHELL_INTEGRATION, fish: FISH_SHELL_INTEGRATION })
}

// ---------------------------------------------------------------- terminalShells
{
  const { isDetectedCustomShell, terminalShellLabel } = await load("lib/terminalShells.ts")
  const t = (key, fallback) => `${key}|${fallback}`
  const profile = (shell, extra = {}) => ({ shell, label: `${shell} label`, source: "src", ...extra })
  record("terminalShells.labels", {
    auto: terminalShellLabel(profile("auto"), t),
    cmd: terminalShellLabel(profile("cmd"), t),
    powershell: terminalShellLabel(profile("powershell"), t),
    pwsh: terminalShellLabel(profile("pwsh"), t),
    wsl: terminalShellLabel(profile("wsl"), t),
    gitBash: terminalShellLabel(profile("git-bash"), t),
    custom: terminalShellLabel(profile("custom", { source: "manual path" }), t),
    detectedCustom: terminalShellLabel(profile("custom", { source: "/bin/zsh" }), t),
    wslDistro: terminalShellLabel(profile("wsl", { wslDistro: "Ubuntu" }), t),
  })
  record("terminalShells.isDetectedCustomShell", {
    manualPath: isDetectedCustomShell(profile("custom", { source: "manual path" })),
    shellVariable: isDetectedCustomShell(profile("custom", { source: "$SHELL" })),
    detected: isDetectedCustomShell(profile("custom", { source: "/bin/zsh" })),
    builtin: isDetectedCustomShell(profile("pwsh")),
  })
}

// ---------------------------------------------------------------- localShells
{
  const {
    buildShellEntries,
    copiedShellCommand,
    customShellKey,
    defaultShellChoice,
    defaultShellKey,
    defaultShellUpdates,
    detectedShellKey,
    failedShellFallback,
    localShellConnection,
    movedLocalShell,
    newLocalShellId,
    ownShellChoice,
    programName,
    removedLocalShell,
    savedLocalShell,
    savedLocalShellUpdates,
    shellChoiceKey,
    startsAsSaved,
    startsShell,
    toggledHiddenShell,
    withCurrentLocalShell,
    withLocalShell,
    AUTO_SHELL_KEY,
  } = await load("lib/localShells.ts")

  const t = (_key, fallback) => fallback
  const nu = { id: "nu", name: "nu", path: "C:\\nu\\nu.exe", args: "" }
  const msys = { id: "msys", name: "MSYS2", path: "C:\\msys64\\usr\\bin\\bash.exe", args: "--login -i" }
  const shellConfig = (overrides = {}) => ({
    terminal_shell: "auto",
    terminal_shell_custom_path: "",
    terminal_shell_custom_args: "",
    terminal_shell_id: "",
    terminal_shell_wsl_distro: "",
    local_shells: [],
    hidden_shells: [],
    ...overrides,
  })
  const windowsShells = [
    { shell: "auto", label: "Auto (recommended)", source: "tTerm default" },
    { shell: "cmd", label: "Command Prompt (cmd)", source: "C:\\Windows\\System32\\cmd.exe" },
    { shell: "pwsh", label: "PowerShell 7 (pwsh)", source: "C:\\PowerShell\\7\\pwsh.exe" },
    { shell: "custom", label: "Custom executable", source: "manual path" },
  ]
  const unixShells = [
    { shell: "auto", label: "Auto (recommended)", source: "tTerm default" },
    { shell: "custom", label: "zsh", source: "/bin/zsh" },
    { shell: "custom", label: "fish", source: "/opt/homebrew/bin/fish" },
    { shell: "custom", label: "Custom executable", source: "manual path" },
  ]

  record("localShells.keys", { AUTO_SHELL_KEY, custom: customShellKey("nu"), wsl: wslKey("Ubuntu") })
  function wslKey(name) {
    return `wsl:${name}`
  }
  record("localShells.detectedShellKey", {
    pwsh: detectedShellKey({ shell: "pwsh", source: "x" }),
    custom: detectedShellKey({ shell: "custom", source: "/bin/zsh" }),
    wsl: detectedShellKey({ shell: "wsl", source: "wsl.exe", wslDistro: "Ubuntu" }),
  })
  record("localShells.programName", {
    quotedWindows: programName('"C:\\Program Files\\nu\\nu.exe"'),
    unix: programName("/opt/homebrew/bin/fish"),
    bare: programName("pwsh"),
    dotted: programName(".hidden"),
  })

  const entries = buildShellEntries(windowsShells, shellConfig({ local_shells: [nu, msys], hidden_shells: ["cmd"] }), t)
  check(
    "localShells.test › buildShellEntries › lists Auto, the shells found here, then the user's own",
    { shell: "windows", hidden: ["cmd"] },
    [
      ["auto", "auto", false],
      ["cmd", "detected", true],
      ["pwsh", "detected", false],
      ["custom:nu", "custom", false],
      ["custom:msys", "custom", false],
    ],
    entries.map((entry) => [entry.key, entry.kind, entry.hidden])
  )
  check(
    "localShells.test › buildShellEntries › the user's shell carries its path, arguments and id",
    "MSYS2",
    {
      label: "MSYS2",
      detail: "C:\\msys64\\usr\\bin\\bash.exe --login -i",
      choice: { shell: "custom", customPath: "C:\\msys64\\usr\\bin\\bash.exe", customArgs: "--login -i", shellId: "msys" },
    },
    { label: entries[4].label, detail: entries[4].detail, choice: entries[4].choice }
  )

  const wsl = "C:\\Windows\\System32\\wsl.exe"
  const distros = [
    { shell: "wsl", label: "Ubuntu", source: wsl, wslDistro: "Ubuntu", defaultWslDistro: true },
    { shell: "wsl", label: "Debian", source: wsl, wslDistro: "Debian" },
  ]
  const keys = (config, profiles = distros) => buildShellEntries(profiles, config, t).map((entry) => entry.key)
  const wslDefault = shellConfig({ terminal_shell: "wsl" })
  const unmarked = distros.map((entry) => ({ ...entry, defaultWslDistro: undefined }))
  const whole = [{ shell: "wsl", label: "WSL", source: wsl }]
  const debian = buildShellEntries(distros, shellConfig(), t)[2]
  check(
    "localShells.test › buildShellEntries › lists each WSL distribution under its own key, and WSL's default while that is the default",
    "wsl",
    {
      plain: ["auto", "wsl:Ubuntu", "wsl:Debian"],
      wslDefault: ["auto", "wsl", "wsl:Ubuntu", "wsl:Debian"],
      wslDefaultEntry: { label: "Windows Subsystem for Linux (WSL)", choice: { shell: "wsl" } },
      defaultKey: "wsl",
      unmarked: ["auto", "wsl", "wsl:Ubuntu", "wsl:Debian"],
      whole: ["auto", "wsl"],
      debian: { label: "Debian", detail: `${wsl} -d Debian`, choice: { shell: "wsl", wslDistro: "Debian" } },
      copied: { path: wsl, args: "-d Debian" },
    },
    {
      plain: keys(shellConfig()),
      wslDefault: keys(wslDefault),
      wslDefaultEntry: subset(buildShellEntries(distros, wslDefault, t)[1], ["label", "choice"]),
      defaultKey: defaultShellKey(wslDefault),
      unmarked: keys(wslDefault, unmarked),
      whole: keys(wslDefault, whole),
      debian: subset(debian, ["label", "detail", "choice"]),
      copied: copiedShellCommand(debian),
    }
  )

  const fish = { id: "f", name: "", path: "/opt/homebrew/bin/fish", args: "" }
  const unixEntries = buildShellEntries(unixShells, shellConfig({ local_shells: [fish] }), t)
  check(
    "localShells.test › buildShellEntries › keys a shell found on PATH by its path and shows one the user added once",
    "unix",
    [
      ["auto", "Auto (recommended)"],
      ["/bin/zsh", "zsh"],
      ["custom:f", "fish"],
    ],
    unixEntries.map((entry) => [entry.key, entry.label])
  )

  check(
    "localShells.test › the default shell › is found by the id of the user's shell, or else by its kind or path",
    "defaults",
    ["auto", "pwsh", "/bin/zsh", "custom:nu"],
    [
      defaultShellKey(shellConfig()),
      defaultShellKey(shellConfig({ terminal_shell: "pwsh" })),
      defaultShellKey(shellConfig({ terminal_shell: "custom", terminal_shell_custom_path: "/bin/zsh" })),
      defaultShellKey(
        shellConfig({
          terminal_shell: "custom",
          terminal_shell_custom_path: "old",
          terminal_shell_id: "nu",
          local_shells: [nu],
        })
      ),
    ]
  )
  check(
    "localShells.test › the default shell › starts with the current path of the user's shell",
    "choices",
    [
      { shell: "custom", customPath: msys.path, customArgs: "--login -i", shellId: "msys" },
      { shell: "cmd" },
    ],
    [
      defaultShellChoice(
        shellConfig({
          terminal_shell: "custom",
          terminal_shell_custom_path: "stale.exe",
          terminal_shell_id: "msys",
          local_shells: [msys],
        })
      ),
      defaultShellChoice(shellConfig({ terminal_shell: "cmd" })),
    ]
  )
  check(
    "localShells.test › the default shell › is set with the path older versions read",
    "updates",
    [
      {
        terminal_shell: "custom",
        terminal_shell_custom_path: "nu",
        terminal_shell_custom_args: "-l",
        terminal_shell_id: "nu",
        terminal_shell_wsl_distro: "",
      },
      {
        terminal_shell: "pwsh",
        terminal_shell_custom_path: "",
        terminal_shell_custom_args: "",
        terminal_shell_id: "",
        terminal_shell_wsl_distro: "",
      },
    ],
    [
      defaultShellUpdates({ shell: "custom", customPath: " nu ", customArgs: "-l", shellId: "nu" }),
      defaultShellUpdates({ shell: "pwsh", customPath: "x" }),
    ]
  )
  const debianUpdates = defaultShellUpdates({ shell: "wsl", wslDistro: "Debian" })
  check(
    "localShells.test › the default shell › can be a WSL distribution, or WSL's own default",
    "wsl",
    {
      distro: "Debian",
      key: "wsl:Debian",
      choice: { shell: "wsl", wslDistro: "Debian" },
      defaultKey: "wsl",
      defaultChoice: { shell: "wsl" },
    },
    {
      distro: debianUpdates.terminal_shell_wsl_distro,
      key: defaultShellKey(shellConfig(debianUpdates)),
      choice: defaultShellChoice(shellConfig(debianUpdates)),
      defaultKey: defaultShellKey(shellConfig({ terminal_shell: "wsl" })),
      defaultChoice: defaultShellChoice(shellConfig({ terminal_shell: "wsl" })),
    }
  )

  const edited = { ...nu, path: "D:\\nu.exe", args: "-l" }
  check(
    "localShells.test › the user's shells › keep the default's path in step when the default is edited",
    "edit",
    [
      { local_shells: [edited], terminal_shell_custom_path: "D:\\nu.exe", terminal_shell_custom_args: "-l" },
      { local_shells: [nu, msys] },
    ],
    [
      savedLocalShell(
        shellConfig({ terminal_shell: "custom", terminal_shell_custom_path: nu.path, terminal_shell_id: "nu", local_shells: [nu] }),
        edited
      ),
      savedLocalShell(shellConfig({ local_shells: [nu] }), msys),
    ]
  )
  check(
    "localShells.test › the user's shells › leave Auto as the default when the default is removed",
    "remove",
    [
      {
        local_shells: [msys],
        terminal_shell: "auto",
        terminal_shell_custom_path: "",
        terminal_shell_custom_args: "",
        terminal_shell_id: "",
        terminal_shell_wsl_distro: "",
      },
      { local_shells: [nu] },
    ],
    [
      removedLocalShell(shellConfig({ terminal_shell: "custom", terminal_shell_id: "nu", local_shells: [nu, msys] }), "nu"),
      removedLocalShell(shellConfig({ terminal_shell: "custom", terminal_shell_id: "nu", local_shells: [nu, msys] }), "msys"),
    ]
  )
  check(
    "localShells.test › the user's shells › move one place at a time and stop at either end",
    "move",
    [[msys, nu], [nu, msys], [nu, msys]],
    [movedLocalShell([nu, msys], "msys", -1), movedLocalShell([nu, msys], "nu", -1), movedLocalShell([nu, msys], "missing", 1)]
  )
  check(
    "localShells.test › the user's shells › hide and show detected shells by key",
    "hidden",
    [["cmd", "pwsh"], ["pwsh"]],
    [toggledHiddenShell(["cmd"], "pwsh"), toggledHiddenShell(["cmd", "pwsh"], "cmd")]
  )
  record("localShells.newId", { looksLikeUuid: /^[0-9a-f-]{36}$/.test(newLocalShellId()) })

  const connection = localShellConnection({ shell: "custom", customPath: nu.path, shellId: "nu" }, "C:\\src")
  const moved = { ...nu, path: "D:\\nu.exe" }
  check(
    "localShells.test › local tab connections › carry the user's shell id so a restart follows its edits",
    "connection",
    {
      connection: {
        type: "terminal",
        cwd: "C:\\src",
        terminalShell: "custom",
        terminalShellCustomPath: nu.path,
        terminalShellCustomArgs: undefined,
        terminalShellId: "nu",
      },
      moved: "D:\\nu.exe",
      kept: connection,
    },
    {
      connection,
      moved: withCurrentLocalShell(connection, [moved])?.terminalShellCustomPath,
      kept: withCurrentLocalShell(connection, []),
    }
  )
  check(
    "localShells.test › local tab connections › leave out custom fields for a built-in shell",
    "pwsh",
    {
      type: "terminal",
      cwd: undefined,
      terminalShell: "pwsh",
      terminalShellCustomPath: undefined,
      terminalShellCustomArgs: undefined,
      terminalShellId: undefined,
    },
    localShellConnection({ shell: "pwsh", customPath: "x", shellId: "y" })
  )

  check(
    "localShells.test › programName › names a shell after its program",
    "names",
    ["nu", "fish", "pwsh", ".hidden"],
    [programName('"C:\\Program Files\\nu\\nu.exe"'), programName("/opt/homebrew/bin/fish"), programName("pwsh"), programName(".hidden")]
  )

  const own = { shell: "custom", customPath: nu.path, shellId: "nu" }
  const debianChoice = { shell: "wsl", wslDistro: "Debian" }
  check(
    "localShells.test › restarting a local tab with another shell › tells whether a tab starts the same shell",
    "startsShell",
    [true, false, true, false, true, false, true],
    [
      startsShell(undefined, { shell: "auto" }),
      startsShell({ type: "terminal", terminalShell: "pwsh" }, { shell: "cmd" }),
      startsShell(localShellConnection(own), own),
      startsShell(localShellConnection(own), { ...own, shellId: "other" }),
      startsShell(localShellConnection(debianChoice), debianChoice),
      startsShell(localShellConnection(debianChoice), { shell: "wsl" }),
      startsShell(
        { type: "terminal", terminalShell: "custom", terminalShellCustomPath: "/bin/zsh " },
        { shell: "custom", customPath: "/bin/zsh" }
      ),
    ]
  )
  const tab = { connection: localShellConnection({ shell: "custom", customPath: "gone" }, "/src"), sessionNonce: 3 }
  check(
    "localShells.test › restarting a local tab with another shell › keeps the tab's directory and starts a new session",
    "withLocalShell",
    { connection: localShellConnection({ shell: "auto" }, "/src"), sessionNonce: 4 },
    withLocalShell(tab, { shell: "auto" })
  )
  const msysTab = localShellConnection(ownShellChoice(msys))
  check(
    "localShells.test › restarting a local tab with another shell › tells whether a tab starts the user's shell as it is saved now",
    "startsAsSaved",
    [true, true, false, false],
    [
      startsAsSaved(msysTab, msys),
      startsAsSaved(msysTab, { ...msys, args: " --login -i " }),
      startsAsSaved(msysTab, { ...msys, args: "--login" }),
      startsAsSaved(msysTab, { ...msys, path: "D:\\msys64\\usr\\bin\\bash.exe" }),
    ]
  )
  const nuDefault = shellConfig({
    terminal_shell: "custom",
    terminal_shell_custom_path: nu.path,
    terminal_shell_id: "nu",
    local_shells: [nu],
  })
  check(
    "localShells.test › restarting a local tab with another shell › offers the default shell, or Auto when the default is the one that failed",
    "failedShellFallback",
    [
      { choice: ownShellChoice(nu), replacesDefault: false },
      { choice: { shell: "auto" }, replacesDefault: true },
      { choice: { shell: "auto", customPath: undefined, customArgs: undefined }, replacesDefault: false },
      undefined,
    ],
    [
      failedShellFallback(msysTab, nuDefault),
      failedShellFallback(localShellConnection(ownShellChoice(nu)), nuDefault),
      failedShellFallback(msysTab, shellConfig()),
      failedShellFallback(localShellConnection({ shell: "auto" }), shellConfig()),
    ]
  )
  check(
    "localShells.test › restarting a local tab with another shell › saves a shell and makes it the default in one change",
    "savedLocalShellUpdates",
    [
      {
        local_shells: [nu],
        terminal_shell: "custom",
        terminal_shell_custom_path: nu.path,
        terminal_shell_custom_args: "",
        terminal_shell_id: "nu",
        terminal_shell_wsl_distro: "",
      },
      { local_shells: [nu] },
    ],
    [savedLocalShellUpdates(shellConfig(), nu, true), savedLocalShellUpdates(shellConfig(), nu, false)]
  )
  record("localShells.shellChoiceKey", {
    nu: shellChoiceKey(ownShellChoice(nu), shellConfig({ local_shells: [nu] })),
    debian: shellChoiceKey(debianChoice, shellConfig()),
  })
  record("localShells.extra", {
    sessionNonceWrap: withLocalShell({ connection: undefined, sessionNonce: 4294967295 }, { shell: "auto" }).sessionNonce,
    programNameTrailingSpace: programName("nu .exe"),
    programNameTwoQuotes: programName('""nu.exe""'),
  })
}

// ------------------------------------------------------------------ appConfig
const appConfig = await load("lib/appConfig.ts")
{
  const config = appConfig
  record("appConfig.defaultConfig", config.defaultConfig)
  record("appConfig.constants", {
    DEFAULT_TERMINAL_FONT_SIZE: config.DEFAULT_TERMINAL_FONT_SIZE,
    DEFAULT_TERMINAL_FONT_FAMILY: config.DEFAULT_TERMINAL_FONT_FAMILY,
    DEFAULT_PREDICTIVE_ECHO_THRESHOLD_MS: config.DEFAULT_PREDICTIVE_ECHO_THRESHOLD_MS,
    PREDICTIVE_ECHO_THRESHOLD_MIN_MS: config.PREDICTIVE_ECHO_THRESHOLD_MIN_MS,
    PREDICTIVE_ECHO_THRESHOLD_MAX_MS: config.PREDICTIVE_ECHO_THRESHOLD_MAX_MS,
    HIDDEN_RENDERER_RELEASE_SECS_OPTIONS: config.HIDDEN_RENDERER_RELEASE_SECS_OPTIONS,
    DEFAULT_HIDDEN_RENDERER_RELEASE_SECS: config.DEFAULT_HIDDEN_RENDERER_RELEASE_SECS,
    TERMINAL_LOG_MAX_RETENTION_DAYS: config.TERMINAL_LOG_MAX_RETENTION_DAYS,
    TERMINAL_LOG_MAX_TOTAL_MB: config.TERMINAL_LOG_MAX_TOTAL_MB,
    MAX_BANDWIDTH_LIMIT_KIB: config.MAX_BANDWIDTH_LIMIT_KIB,
    RECONNECT_MAX_ATTEMPTS_MIN: config.RECONNECT_MAX_ATTEMPTS_MIN,
    RECONNECT_MAX_ATTEMPTS_MAX: config.RECONNECT_MAX_ATTEMPTS_MAX,
    TERMINAL_FONT_SIZE_RANGE: config.TERMINAL_FONT_SIZE_RANGE,
    TERMINAL_LINE_HEIGHT_RANGE: config.TERMINAL_LINE_HEIGHT_RANGE,
    TERMINAL_LETTER_SPACING_RANGE: config.TERMINAL_LETTER_SPACING_RANGE,
    WINDOW_OPACITY_PERCENT_RANGE: config.WINDOW_OPACITY_PERCENT_RANGE,
    WINDOW_BLUR_RADIUS_RANGE: config.WINDOW_BLUR_RADIUS_RANGE,
    NOTIFY_COMMAND_MIN_SECS_RANGE: config.NOTIFY_COMMAND_MIN_SECS_RANGE,
    TOAST_MAX_VISIBLE_RANGE: config.TOAST_MAX_VISIBLE_RANGE,
    NOTIFICATION_SOUND_NAMES: config.NOTIFICATION_SOUND_NAMES,
    defaultRenderer: config.getDefaultTerminalRenderer(),
    defaultAppState: config.defaultAppState,
    defaultSecretStatus: config.defaultSecretStatus,
  })
  record("appConfig.platformDefaults", {
    platform: PLATFORM,
    font_family: config.defaultConfig.font_family,
    right_click_paste: config.defaultConfig.right_click_paste,
    local_unix_shell_integration: config.defaultConfig.local_unix_shell_integration,
    update_channel: config.defaultConfig.update_channel,
    language: config.defaultConfig.language,
  })
  record("appConfig.fieldCount", Object.keys(config.defaultConfig).length)
}

// ------------------------------------------------------- appConfigNormalize
{
  const normalize = await load("lib/appConfigNormalize.ts")
  const { defaultConfig, defaultAppState, defaultSecretStatus } = appConfig
  const config = normalize.normalizeConfig

  record("appConfigNormalize.defaultVsEmpty", {
    keys: Object.keys(defaultConfig).length,
    differs: Object.keys(defaultConfig).filter(
      (key) => JSON.stringify(defaultConfig[key]) !== JSON.stringify(config({})[key])
    ),
  })
  record("appConfigNormalize.extraKeysKept", Object.keys(config({ future_setting: 1, tab_icons: { enabled: false } })).filter((key) =>
    ["future_setting", "tab_icons"].includes(key)
  ))

  const pick = (input, keys) => Object.fromEntries(keys.map((key) => [key, config(input)[key]]))
  record("appConfigNormalize.normalize", {
    empty: pick({}, ["theme", "ui_style", "font_size", "scrollback_lines", "secret_storage_mode"]),
    booleans: pick(
      {
        theme_follow_system: "true",
        restore_terminal_content: false,
        resume_agent_sessions: false,
        sftp_paste_upload_enabled: true,
        auto_download_updates: false,
        reconnect_enabled: false,
        zmodem_auto_detect_enabled: false,
        password_feedback: false,
        mac_option_is_meta: true,
        confirm_multiline_paste: false,
        copy_on_select: true,
        osc52_clipboard_write: false,
        command_marks: false,
        ssh_shell_integration: false,
        show_jump_host_connection_info: false,
      },
      [
        "theme_follow_system",
        "restore_terminal_content",
        "resume_agent_sessions",
        "sftp_paste_upload_enabled",
        "auto_download_updates",
        "reconnect_enabled",
        "zmodem_auto_detect_enabled",
        "password_feedback",
        "mac_option_is_meta",
        "confirm_multiline_paste",
        "copy_on_select",
        "osc52_clipboard_write",
        "command_marks",
        "ssh_shell_integration",
        "show_jump_host_connection_info",
      ]
    ),
    themes: pick(
      { theme_light: "", theme_dark: "nord", favorite_themes: ["a", "a", "", 3, "b"] },
      ["theme_light", "theme_dark", "favorite_themes"]
    ),
    startup: pick({ startup_session_restore_mode: "all" }, ["startup_session_restore_mode"]),
    sftp: pick(
      { sftp_start_directory: "home", sftp_external_editor: "  code  ", sftp_external_upload: "auto" },
      ["sftp_start_directory", "sftp_external_editor", "sftp_external_upload"]
    ),
    sftpFallbacks: pick(
      { sftp_start_directory: "bogus", sftp_external_upload: "x" },
      ["sftp_start_directory", "sftp_external_upload"]
    ),
    secret: {
      vault: config({ secret_storage_mode: "vault" }).secret_storage_mode,
      password: config({ secret_storage_mode: "password" }).secret_storage_mode,
      memory: config({ secret_storage_mode: "memory" }).secret_storage_mode,
      hybrid: config({ secret_storage_mode: "hybrid" }).secret_storage_mode,
      other: config({ secret_storage_mode: "x" }).secret_storage_mode,
      unlock: config({ prompt_unlock_vault_on_startup: true }).prompt_unlock_vault_on_startup,
    },
    renderer: {
      canvas: config({ terminal_renderer: "canvas" }).terminal_renderer,
      unknown: config({ terminal_renderer: "x" }).terminal_renderer,
    },
    hiddenRendererRelease: [30, 20, "30", null].map(
      (value) => config({ hidden_renderer_release_secs: value }).hidden_renderer_release_secs
    ),
    monitor: {
      refresh: [-1, 100, 5.6, "5", Number.NaN].map(
        (value) => config({ monitor_refresh_interval_secs: value }).monitor_refresh_interval_secs
      ),
      metrics: config({ monitor_visible_metrics: ["load", "load", "x", 1] }).monitor_visible_metrics,
      metricsEmpty: config({ monitor_visible_metrics: [] }).monitor_visible_metrics,
    },
    updates: pick({ update_channel: "beta-dev", update_check_frequency: "weekly" }, ["update_channel", "update_check_frequency"]),
    updatesFallback: pick({ update_channel: "x", update_check_frequency: "x" }, ["update_channel", "update_check_frequency"]),
    updateCheckEvery3Days: pick({ update_check_frequency: "every-3-days" }, ["update_check_frequency"]),
    updateCheckNever: pick({ update_check_frequency: "never" }, ["update_check_frequency"]),
    padding: {
      left: [6.4, -3, 100, "6", Number.NaN].map((value) => config({ terminal_padding_left_px: value }).terminal_padding_left_px),
      right: config({ terminal_padding_right_px: 80.6 }).terminal_padding_right_px,
      bottom: config({ terminal_padding_bottom_px: null }).terminal_padding_bottom_px,
    },
    logs: {
      enabled: config({ terminal_log_enabled: true }).terminal_log_enabled,
      directory: config({ terminal_log_directory: "/tmp/logs" }).terminal_log_directory,
      formatRaw: config({ terminal_log_format: "raw" }).terminal_log_format,
      formatPlain: config({ terminal_log_format: "plain" }).terminal_log_format,
      formatFallback: config({ terminal_log_format: "x" }).terminal_log_format,
      templateBlank: config({ terminal_log_name_template: "   " }).terminal_log_name_template,
      templateTrimmed: config({ terminal_log_name_template: " a " }).terminal_log_name_template,
      fileSize: [0, 2000, 1.5, "5"].map(
        (value) => config({ terminal_log_max_file_size_mb: value }).terminal_log_max_file_size_mb
      ),
      retention: [-1, 3.6, 4000, "3"].map(
        (value) => config({ terminal_log_retention_days: value }).terminal_log_retention_days
      ),
      total: [2.5, -2, 2_000_000].map(
        (value) => config({ terminal_log_max_total_mb: value }).terminal_log_max_total_mb
      ),
      compress: config({ terminal_log_compress: true }).terminal_log_compress,
      plainTimestamps: config({ terminal_log_plain_timestamps: false }).terminal_log_plain_timestamps,
      recordInput: config({ terminal_log_record_input: true }).terminal_log_record_input,
    },
    sftpTransfer: {
      parallelism: [0, 99, 4.5, "4"].map(
        (value) => config({ sftp_transfer_parallelism: value }).sftp_transfer_parallelism
      ),
      bandwidth: [1_000_000_000, -5, 2.5].map((value) => config({ sftp_upload_limit_kib: value }).sftp_upload_limit_kib),
      download: config({ sftp_download_limit_kib: 1 }).sftp_download_limit_kib,
    },
    reconnect: [0, 100, 2.5, "2"].map((value) => config({ reconnect_max_attempts: value }).reconnect_max_attempts),
    zmodemDirectory: config({ zmodem_download_directory: " /tmp " }).zmodem_download_directory,
    proxy: {
      trimmed: config({ default_proxy_id: "  p1  " }).default_proxy_id,
      missing: config({ default_proxy_id: 3 }).default_proxy_id,
    },
    sudo: config({ sudo_prompt_patterns: ["a", " ", "", 3, "b"] }).sudo_prompt_patterns,
    predictive: {
      mode: ["always", "off", "x", undefined].map((value) => config({ predictive_echo: value }).predictive_echo),
      threshold: [5, 5000, 30.6, "30"].map(
        (value) => config({ predictive_echo_threshold_ms: value }).predictive_echo_threshold_ms
      ),
    },
    uiScale: [84, 86, 300, 10, "100", Number.NaN].map(
      (value) => config({ ui_scale_percent: value }).ui_scale_percent
    ),
    lineHeight: [1.234, 0.5, 3, "1.2"].map(
      (value) => config({ terminal_line_height: value }).terminal_line_height
    ),
    letterSpacing: [-99, 99, 3.5, "3"].map(
      (value) => config({ terminal_letter_spacing: value }).terminal_letter_spacing
    ),
    keymap: {
      empty: config({ keymap: {} }).keymap,
      garbage: config({ keymap: { bindings: { nope: ["mod+k"] } } }).keymap,
      null: config({ keymap: null }).keymap,
    },
    background: {
      throttling: ["disabled", "suspend", "x"].map(
        (value) => config({ background_throttling: value }).background_throttling
      ),
    },
    window: {
      blur: [true, "true"].map((value) => config({ window_blur: value }).window_blur),
      radius: [0, 100, 20.6].map((value) => config({ window_blur_radius: value }).window_blur_radius),
      opacity: [10, 100, 70.4].map((value) => config({ window_opacity_percent: value }).window_opacity_percent),
      material: ["mica", "x"].map((value) => config({ window_blur_material: value }).window_blur_material),
    },
    style: {
      ui_style: ["islands", "classic", "x"].map((value) => config({ ui_style: value }).ui_style),
      profile_sort: ["recent", "x"].map((value) => config({ profile_sort: value }).profile_sort),
      close_behavior: ["tray", "quit", "x"].map((value) => config({ close_behavior: value }).close_behavior),
    },
    notifications: {
      enabled: config({ notifications_enabled: false }).notifications_enabled,
      finished: config({ notify_command_finished: false }).notify_command_finished,
      minSecs: [0, 4000, 10.6].map((value) => config({ notify_command_min_secs: value }).notify_command_min_secs),
      terminalRequests: config({ notify_terminal_requests: false }).notify_terminal_requests,
      agentWaiting: config({ notify_agent_waiting: false }).notify_agent_waiting,
      agentDone: config({ notify_agent_done: false }).notify_agent_done,
      agentConfigDirs: config({ agent_config_dirs: { a: "1", b: "", c: 1, d: "2" } }).agent_config_dirs,
      agentConfigDirsBad: config({ agent_config_dirs: ["x"] }).agent_config_dirs,
      bellNotify: config({ bell_notify: true }).bell_notify,
      toast: [0, 9, 3.6].map((value) => config({ toast_max_visible: value }).toast_max_visible),
      sound: config({ notify_sound: false }).notify_sound,
      soundName: ["chime", "custom", "x"].map((value) => config({ notify_sound_name: value }).notify_sound_name),
      soundFile: config({ notify_sound_file: 3 }).notify_sound_file,
      bellStyle: ["visual", "sound", "x"].map((value) => config({ bell_style: value }).bell_style),
    },
    tabs: config({ tab_width_mode: "standard", tab_standard_width: 999, panes: { dim_inactive_percent: 70 } }),
    keywordHighlight: {
      missing: config({ keyword_highlight: undefined }).keyword_highlight,
      disabled: config({ keyword_highlight: { enabled: false } }).keyword_highlight,
    },
    rawFields: {
      font_size: config({ font_size: "14" }).font_size,
      scrollback_lines: config({ scrollback_lines: -1 }).scrollback_lines,
      terminal_shell: config({ terminal_shell: "bogus" }).terminal_shell,
      terminal_shell_custom_path: config({ terminal_shell_custom_path: "  nu  " }).terminal_shell_custom_path,
      local_shells: config({ local_shells: [{ id: 1 }] }).local_shells,
      hidden_shells: config({ hidden_shells: ["a", 1] }).hidden_shells,
      theme: config({ theme: 3 }).theme,
      language: config({ language: "xx" }).language,
    },
  })

  record("appConfigNormalize.appState", {
    empty: normalize.normalizeAppState({}),
    values: normalize.normalizeAppState({ lastUpdateCheckAt: 5, collapsedProfileGroupKeys: ["a", 1, "a"] }),
    nan: String(normalize.normalizeAppState({ lastUpdateCheckAt: Number.NaN }).lastUpdateCheckAt),
    defaults: defaultAppState,
  })
  record("appConfigNormalize.secretStatus", {
    empty: normalize.normalizeSecretStatus(),
    partial: normalize.normalizeSecretStatus({ storageMode: "vault", message: "hi", verificationMethod: "password" }),
    unknownVerification: normalize.normalizeSecretStatus({ verificationMethod: "x" }),
    defaults: defaultSecretStatus,
  })
  // `normalizeConfig` forces the channel of a config that does not name one to
  // "stable", while `defaultConfig` follows the running version, so a
  // prerelease build shows the one field where the two disagree.
  record("appConfigNormalize.prereleaseChannel", {
    defaultConfig: defaultConfig.update_channel,
    normalized: config({}).update_channel,
  })
  check(
    "appConfigNormalize › normalizeConfig fills in the defaults",
    {},
    { ...defaultConfig, ui_style: "classic", update_channel: "stable" },
    config({})
  )
}

// ------------------------------------------------------------------ startup
{
  const startup = await load("lib/startup.ts")
  shim.reset()
  let ready = 0
  const unsubscribe = startup.onAppReady(() => ready++)
  startup.markConfigReady()
  startup.markSessionReady()
  startup.announceThemeReady()
  check(
    "startup › onAppReady fires once when config, theme and session are all ready",
    "all ready",
    1,
    ready
  )
  startup.markConfigReady()
  check("startup › onAppReady does not fire twice for the same boot", "again", 1, ready)
  unsubscribe()
  shim.reset()
  let second = 0
  startup.onAppReady(() => second++)
  startup.onAppReady(() => second++)
  check(
    "startup › a second subscription after the splash hid never fires",
    "subscribed again",
    0,
    second
  )
}

// ------------------------------------------------------------ defaults parity
// The Rust `AppConfig::default()` already exists; this checks that it holds the
// same values as the TypeScript `defaultConfig`. It parses the small `Default`
// implementation rather than running Rust so the oracle stays a single command.
{
  const { defaultTerminalFontFamily } = await load("lib/terminalFont.ts")
  const candidates = [
    "src-tauri/src/config/defaults.rs",
    "crates/tterm-core/src/config/defaults.rs",
  ]
  let source = null
  let sourcePath = null
  for (const candidate of candidates) {
    try {
      source = await readFile(`${ROOT}/${candidate}`, "utf8")
      sourcePath = candidate
      break
    } catch {}
  }

  const fontFamilies = {
    macos: defaultTerminalFontFamily("macos"),
    windows: defaultTerminalFontFamily("windows"),
    linux: defaultTerminalFontFamily("linux"),
  }
  const fnTable = {
    default_theme_light: "light",
    default_theme_dark: "default",
    default_language: "en",
    default_font_family: fontFamilies[PLATFORM] ?? fontFamilies.linux,
    default_font_size: 14,
    default_ui_scale_percent: 100,
    default_window_blur_radius: 20,
    default_window_blur_material: "acrylic",
    default_window_opacity_percent: 70,
    default_cursor_style: "block",
    default_terminal_shell: "auto",
    // The backend keeps the older default so the password migration can tell
    // where passwords were kept; the frontend maps it to "system".
    default_secret_storage_mode: PLATFORM === "windows" ? "system" : "hybrid",
    default_secret_vault_enabled: true,
    default_scrollback_lines: 10_000,
    default_terminal_renderer: "webgl",
    default_hidden_renderer_release_secs: 10,
    default_terminal_padding_left_px: 6,
    default_terminal_padding_right_px: 6,
    default_terminal_padding_bottom_px: 4,
    default_startup_session_restore_mode: "active",
    default_show_jump_host_connection_info: true,
    default_password_feedback: true,
    default_predictive_echo: "auto",
    default_predictive_echo_threshold_ms: 30,
    default_monitor_refresh_interval_secs: 5,
    default_monitor_visible_metrics: ["cpu", "memory", "network", "ip", "latency", "disk"],
    default_auto_download_updates: true,
    default_update_check_frequency: "daily",
    default_tab_width_mode: "adaptive",
    default_tab_standard_width: 120,
    default_terminal_log_format: "both",
    default_terminal_log_name_template: "{year}-{month}-{day}/{profile}-{host}-{time}-{sessionId}",
    default_terminal_log_max_file_size_mb: 50,
    default_sftp_start_directory: "last",
    default_sftp_external_upload: "ask",
    default_sftp_transfer_parallelism: 4,
    default_reconnect_enabled: true,
    default_right_click_paste: PLATFORM === "windows",
    default_confirm_multiline_paste: true,
    default_osc52_clipboard_write: true,
    default_command_marks: true,
    default_local_unix_shell_integration: PLATFORM === "macos",
    default_ssh_shell_integration: true,
    default_terminal_line_height: 1,
    default_reconnect_max_attempts: 5,
    default_background_throttling: "throttle",
    default_ui_style: "classic",
    default_close_behavior: "ask",
    default_true: true,
    default_notify_command_min_secs: 10,
    default_toast_max_visible: 3,
    default_notify_sound_name: "system",
    default_bell_style: "none",
    default_zmodem_auto_detect_enabled: true,
    default_profile_sort: "default",
    // The backend follows the running version, like the frontend default does.
    default_update_channel: prereleaseVersion(VERSION) ? "beta-dev" : "stable",
  }
  const constructorTable = {
    "KeymapConfig::default()": { bindings: {} },
    "tab_icons::TabIconConfig::default()": { enabled: true, agents: true, shells: {} },
    "tab_agent_indicator::TabAgentIndicatorConfig::default()": {
      working: "ring",
      waiting: "bars",
      working_color: "",
      waiting_color: "",
    },
    "keyword_highlight::KeywordHighlightConfig::default()": {
      enabled: true,
      default_set: "builtin",
      alternate_screen: false,
      sets: null,
    },
    "panes::PaneConfig::default()": { show_headers: true, dim_inactive_percent: 0, templates: [] },
    "Vec::new()": [],
    "String::new()": "",
    "Default::default()": {},
  }

  if (source === null) {
    record("defaultsParity", { source: null, fields: 0, diffs: [] })
  } else {
    const start = source.indexOf("impl Default for AppConfig")
    const block = source.slice(start)
    const rust = {}
    const unparsed = []
    for (const match of block.matchAll(/^\s{12}(\w+): (.+),$/gm)) {
      const field = match[1]
      const expression = match[2].trim()
      let value
      if (expression.startsWith('"') && expression.endsWith('.to_string()')) {
        value = JSON.parse(expression.slice(0, -'.to_string()'.length))
      } else if (/^-?\d+(\.\d+)?$/.test(expression)) {
        value = Number(expression)
      } else if (expression === "true" || expression === "false") {
        value = expression === "true"
      } else if (expression in constructorTable) {
        value = constructorTable[expression]
      } else if (`${expression.replace("()", "")}` in fnTable) {
        value = fnTable[expression.replace("()", "")]
      } else {
        unparsed.push(`${field}: ${expression}`)
        continue
      }
      rust[field] = value
    }
    // Kept for the one-time password migration and never serialized.
    delete rust.legacy_local_shell_integration
    const tsConfig = appConfig.defaultConfig
    const diffs = []
    for (const key of new Set([...Object.keys(tsConfig), ...Object.keys(rust)])) {
      if (JSON.stringify(tsConfig[key]) !== JSON.stringify(rust[key])) {
        diffs.push({ key, frontend: tsConfig[key], backend: rust[key] })
      }
    }
    record("defaultsParity", { source: sourcePath, fields: Object.keys(rust).length, unparsed, diffs })
    check(
      "defaults parity › the backend default matches the frontend except the documented password storage mode",
      PLATFORM,
      PLATFORM === "windows" ? [] : ["secret_storage_mode"],
      diffs.map((entry) => entry.key)
    )
    check("defaults parity › every default of the backend is parsed", [], [], unparsed)
  }
}

// -------------------------------------------------------------------- report
const failed = cases.filter((entry) => !entry.pass)
const output = {
  environment: {
    node: process.version,
    platform: PLATFORM,
    packageVersion: VERSION,
    ranAt: new Date().toISOString(),
  },
  summary: {
    cases: cases.length,
    passed: cases.length - failed.length,
    failed: failed.length,
  },
  cases,
  values,
}
console.log(JSON.stringify(output, null, 2))
if (failed.length > 0) {
  console.error(`${failed.length} case(s) failed`)
  process.exit(1)
}
