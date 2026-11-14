// Runs the real TypeScript modules of P1-11g in Node, without a browser and
// without touching src/. Used by the plan to record the exact values the Rust
// ports must reproduce. Run from the repository root:
//
//   node docs/gpui-migration/reports/P1-11g-evidence/ts-oracle.mjs > docs/gpui-migration/reports/P1-11g-evidence/ts-oracle.txt
import fs from "node:fs"
import ts from "typescript"

const cache = new Map()

const stubs = {
  react: `export const useSyncExternalStore = () => ({})
export default {}`,
  "@tauri-apps/api/core": `export class Channel { onmessage }
export const invoke = async () => { throw new Error("invoke is not stubbed") }`,
  "@tauri-apps/api/app": `export const getVersion = async () => "0.0.0"`,
  "@tauri-apps/plugin-process": `export const relaunch = async () => {}`,
  "@tauri-apps/plugin-dialog": `export const open = async () => null`,
  "@tauri-apps/plugin-os": `export const platform = () => "macos"`,
  "@/styles/tokens.json": `export default {}`,
  clsx: `export const clsx = (...values) => values.flat().filter(Boolean).join(" ")`,
  "tailwind-merge": `export const twMerge = (value) => value`,
  "@/lib/utils": `export function toErrorMessage(error) {
  return error instanceof Error ? error.message : String(error)
}`,
}

function dataUrl(source) {
  return `data:text/javascript;base64,${Buffer.from(source).toString("base64")}`
}

function resolveFile(specifier) {
  const base = specifier.replace(/^@\//, "src/")
  for (const candidate of [base, `${base}.ts`, `${base}.tsx`, `${base}/index.ts`, `${base}/index.tsx`]) {
    if (fs.existsSync(candidate) && fs.statSync(candidate).isFile()) return candidate
  }
  throw new Error(`cannot resolve ${specifier}`)
}

// The settings pages carry lucide icons, which the search never reads; the
// page definitions are reproduced from the real file so labels stay real.
function settingsTabsStub() {
  const source = fs.readFileSync("src/components/SettingsDialog/settingsTabs.ts", "utf8")
  const tabs = []
  const pattern = /\{\s*value: "([^"]+)",\s*labelKey: "([^"]+)"(?:,\s*defaultLabel: "([^"]+)")?/g
  for (const match of source.matchAll(pattern)) {
    tabs.push({ value: match[1], labelKey: match[2], defaultLabel: match[3] })
  }
  return `export const SETTINGS_TABS = ${JSON.stringify(tabs)}`
}

function bundle(specifier) {
  if (stubs[specifier]) return dataUrl(stubs[specifier])
  if (specifier === "@/components/SettingsDialog/settingsTabs") return dataUrl(settingsTabsStub())
  const path = resolveFile(specifier)
  if (cache.has(path)) return cache.get(path)
  const output = ts.transpileModule(fs.readFileSync(path, "utf8"), {
    compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ESNext },
  }).outputText
  const specifiers = new Set()
  for (const match of output.matchAll(/(?:from|import)\s*"([^"]+)"/g)) specifiers.add(match[1])
  let source = output
  for (const dependency of specifiers) {
    source = source.replaceAll(
      JSON.stringify(dependency),
      JSON.stringify(bundle(dependency))
    )
  }
  const url = dataUrl(source)
  cache.set(path, url)
  return url
}

const load = (specifier) => import(bundle(specifier))

const print = (title, value) => console.log(`${title}: ${JSON.stringify(value)}`)
const section = (title) => console.log(`\n## ${title}`)

const quickConnect = await load("@/lib/quickConnect")
const profileConnections = await load("@/lib/profileConnections")
const profileGroups = await load("@/lib/profileGroups")
const profileSearch = await load("@/lib/profileSearch")
const profileRecents = await load("@/lib/profileRecents")
const recentCommands = await load("@/lib/recentCommands")
const commandVariables = await load("@/lib/commandVariables")
const composerRun = await load("@/lib/composerRun")
const proxy = await load("@/lib/proxy")
const serial = await load("@/lib/serial")
const sftpStartDirectory = await load("@/lib/sftpStartDirectory")
const monitorMetrics = await load("@/lib/monitorMetrics")
const optionSearch = await load("@/lib/optionSearch")
const terminalLogReplay = await load("@/lib/terminalLogReplay")
const terminalCd = await load("@/lib/terminalCd")
const droppedPaths = await load("@/lib/droppedPaths")
const externalEditor = await load("@/lib/externalEditor")
const notificationBadge = await load("@/lib/notificationBadge")
const vaultLock = await load("@/lib/vaultLock")
const userVerification = await load("@/lib/userVerification")
const settingsSearch = await load("@/components/SettingsDialog/settingsSearch")
const sftpView = await load("@/components/SftpDrawer/sftpView")
const sftpHistory = await load("@/components/SftpDrawer/sftpHistory")
const sftpArchive = await load("@/components/SftpDrawer/sftpArchive")
const sftpSearch = await load("@/components/SftpDrawer/sftpSearch")
const drawerUtils = await load("@/components/SftpDrawer/sftpDrawerUtils")

section("quickConnect")
for (const input of [
  "root@10.0.0.5",
  "root@10.0.0.5:2222",
  "admin@[2001:db8::1]:2200",
  "fe80::1",
  "ssh -p 2222 deploy@web",
  "ssh -l deploy -p2222 web",
  "ssh -J jump@bastion:2200 -i ~/.ssh/k root@db",
  "web.",
  "user@@host",
  "root@host:0",
  "root@host:022",
  "ssh -p 22",
  "[::1]",
  "a/b",
  "",
]) {
  print(`parseQuickConnectInput(${JSON.stringify(input)})`, quickConnect.parseQuickConnectInput(input))
}
print("formatEndpoint IPv6 default port", quickConnect.formatEndpoint({ host: "2001:db8::1", port: 22 }))
print("formatEndpoint IPv6 no port", quickConnect.formatEndpoint({ host: "2001:db8::1" }))
print(
  "matchesQuery('İSTANBUL', ['istanbul'])",
  quickConnect.matchesQuery("İSTANBUL", ["istanbul"])
)
print("matchesQuery('WEB 10.0', ['web-01','10.0.0.5'])", quickConnect.matchesQuery("WEB 10.0", ["web-01", "10.0.0.5"]))
const aliasHost = {
  hostPattern: "bastion",
  name: "bastion",
  host: "203.0.113.10",
  port: 2222,
  username: "ops",
  privateKeyPath: "~/.ssh/bastion",
  agentForward: false,
  keepaliveIntervalSecs: 15,
  keepaliveCountMax: 3,
  jumpHosts: [],
  skipped: false,
}
print("resolveQuickConnectTarget alias", quickConnect.resolveQuickConnectTarget({ host: "bastion" }, [aliasHost]))
print(
  "resolveQuickConnectTarget alias with port",
  quickConnect.resolveQuickConnectTarget({ host: "bastion", port: 22 }, [aliasHost])
)
print("resolveQuickConnectTarget unknown user", quickConnect.resolveQuickConnectTarget({ host: "10.0.0.5" }, []))
print(
  "resolveQuickConnectTarget jump username default",
  quickConnect.resolveQuickConnectTarget({ username: "root", host: "db", jumpHost: { host: "bastion" } }, [])
)
print(
  "draftProfileFromTab(password)",
  quickConnect.draftProfileFromTab({
    id: "t",
    isActive: true,
    ...quickConnect.buildQuickConnectTab({ title: "root@db", username: "root", host: "db", port: 22 }),
  })
)
let recents = []
for (let index = 0; index < 10; index += 1) {
  recents = quickConnect.addRecentQuickConnection(
    recents,
    { title: `h${index}`, username: "u", host: `h${index}`, port: 22 },
    index
  )
}
print("recents after 10 adds", recents.map((recent) => recent.host))
print(
  "recents dedupe key with jump hosts",
  quickConnect.addRecentQuickConnection(
    quickConnect.addRecentQuickConnection([], { title: "a", username: "u", host: "h", port: 22 }, 1),
    {
      title: "a2",
      username: "u",
      host: "h",
      port: 22,
      jumpHosts: [{ host: "j", port: 22, username: "u", authMethod: "auto" }],
    },
    2
  ).length
)

section("profileConnections")
const profile = {
  id: "p1",
  name: "web",
  connection_type: "ssh",
  host: "10.0.0.1",
  port: 22,
  username: "root",
  remember_password: false,
  auth_method: "password",
  keepalive_interval_secs: 15,
  keepalive_count_max: 3,
}
print("buildConnectionFromProfile", profileConnections.buildConnectionFromProfile(profile))
print(
  "normalizeSshAuthMethod('publickey')",
  profileConnections.normalizeSshAuthMethod("publickey")
)
print(
  "applyProfileToTab rename of a copy",
  profileConnections.applyProfileToTab(
    {
      id: "t",
      title: "web-2",
      duplicateBaseTitle: "web",
      type: "ssh",
      isActive: true,
      connection: { type: "ssh", profileId: "p1", profileName: "web", host: "10.0.0.1", port: 22, username: "root" },
    },
    { ...profile, name: "api" }
  ).title
)
print("profileTabIcon unknown distro", profileConnections.profileTabIcon({ ...profile, distro: "beos" }))

section("profileGroups")
const groups = [
  { id: "w", name: "Web", parentId: null },
  { id: "a", name: "api", parentId: null },
  { id: "z", name: "Zebra", parentId: null },
  { id: "p", name: "prod", parentId: null },
]
const tree = profileGroups.buildGroupTree(groups, [])
print("tree roots order", tree.roots.map((node) => node.group.name))
print("normalizeGroupPath(' prod /web// ')", profileGroups.normalizeGroupPath(" prod /web// "))
print("normalizeGroupPath('/')", profileGroups.normalizeGroupPath("/"))

section("profileSearch")
print("profileSearchFields", profileSearch.profileSearchFields(profile))
for (const [name, query] of [
  ["web", "WEB"],
  ["web-01", "web"],
  ["prod-web-01", "web"],
  ["webhook-web", "hook web"],
  ["gateway", "10.0"],
  ["gateway", "root"],
]) {
  print(
    `profileMatchScore({name:${JSON.stringify(name)}}, ${JSON.stringify(query)})`,
    profileSearch.profileMatchScore({ ...profile, name }, query)
  )
}
print(
  "rankProfiles order",
  profileSearch
    .rankProfiles(
      [
        { ...profile, id: "user", name: "gateway", username: "web" },
        { ...profile, id: "older", name: "web-02" },
        { ...profile, id: "newer", name: "web-03" },
        { ...profile, id: "exact", name: "web" },
        { ...profile, id: "unused", name: "web-04" },
      ],
      "web",
      new Map([
        ["older", 1],
        ["newer", 2],
      ])
    )
    .map((item) => item.id)
)

section("profileRecents")
let profileRecentsList = []
for (let index = 0; index < 52; index += 1) {
  profileRecentsList = profileRecents.addRecentProfile(profileRecentsList, `p${index}`, index)
}
print("profile recents length and first", [profileRecentsList.length, profileRecentsList[0].profileId])
print("profileLastUsed size", profileRecents.profileLastUsed(profileRecentsList).size)

section("recentCommands")
print("suggestCommandName long", recentCommands.suggestCommandName("x".repeat(60)))
print("suggestCommandName multiline", recentCommands.suggestCommandName("\n\n  docker ps \nmore"))
print(
  "addRecentCommand dedupe",
  recentCommands.addRecentCommand(
    recentCommands.addRecentCommand([], { commandText: "  git status ", profileId: "one", executedAt: 10 }),
    { commandText: "git status", profileId: "one", executedAt: 20 }
  )
)
print(
  "createCommandDraft without profile",
  recentCommands.createCommandDraft("  docker ps  ")
)

section("commandVariables")
print(
  "extractPlaceholderNames",
  commandVariables.extractPlaceholderNames("ssh {{user}}@{{ host }} -p {{port}} # {{user}} {{1st}} {{.State}}")
)
print("parseChoiceOptionsText", commandVariables.parseChoiceOptionsText("dev, prod\nstaging，dev\n\n"))
print(
  "initialVariableValues",
  commandVariables.initialVariableValues([
    { name: "token", label: "t", valueType: "secret", isRequired: true, position: 0 },
    { name: "env", label: "e", valueType: "choice", optionsJson: '["dev","prod"]', defaultValue: "qa", isRequired: true, position: 1 },
    { name: "zone", label: "z", valueType: "choice", optionsJson: '["x"]', isRequired: false, position: 2 },
  ])
)
print(
  "validateVariableValues('0x10')",
  commandVariables.validateVariableValues(
    [{ name: "port", label: "p", valueType: "number", isRequired: true, position: 0 }],
    { port: "0x10" }
  )
)
print(
  "validateVariableValues(' 22 ')",
  commandVariables.validateVariableValues(
    [{ name: "port", label: "p", valueType: "number", isRequired: true, position: 0 }],
    { port: " 22 " }
  )
)
print(
  "renderCommandText masking",
  commandVariables.renderCommandText(
    "login {{token}} {{host}}",
    [
      { name: "token", label: "t", valueType: "secret", isRequired: true, position: 0 },
      { name: "host", label: "h", valueType: "text", isRequired: true, position: 1 },
    ],
    { token: "s3cret-and-longer", host: "a.io" },
    { maskSecrets: true }
  )
)

section("composerRun")
print("runLines heredoc", composerRun.runLines("cat <<EOF\nline\nEOF\necho done"))
print("runLines continuation", composerRun.runLines("echo a \\\nb\n\necho c"))
print("runLines if block", composerRun.runLines("if true; then\n  echo hi\nfi\necho after"))
const run1 = composerRun.finishLine(composerRun.startRun(["a", "b"]), 0)
print("finishLine success advances", run1)
const run2 = composerRun.finishLine(composerRun.startRun(["a", "b"]), 2)
print("finishLine failure holds", run2)
print(
  "promptReached unstarted comment",
  composerRun.promptReached(composerRun.startRun(["# note", "b"]), { kind: "prompt", startedCommand: false })
)
print(
  "promptReached unstarted parse failure",
  composerRun.promptReached(composerRun.startRun(["fi", "b"]), { kind: "prompt", startedCommand: false, status: 2 })
)

section("proxy")
print("normalizeProxyChoice server empty id", proxy.normalizeProxyChoice({ mode: "server", id: "" }))
print("proxyChoiceKey server", proxy.proxyChoiceKey({ mode: "server", id: "a:b" }))
print("parseProxyChoiceKey('server:')", proxy.parseProxyChoiceKey("server:"))
print("hasOnlyKnownProxyTokens", [
  proxy.hasOnlyKnownProxyTokens("nc %h %p 100%%"),
  proxy.hasOnlyKnownProxyTokens("nc %d"),
  proxy.hasOnlyKnownProxyTokens("nc 100%"),
  proxy.hasOnlyKnownProxyTokens("echo %%"),
])
print(
  "validateProxyDraft port 8080.5",
  proxy.validateProxyDraft({ name: "n", kind: "http", host: "h", port: 8080.5 })
)
print("proxyAddress IPv6", proxy.proxyAddress({ host: "::1", port: 1080 }))
print("proxySummary command", proxy.proxySummary({ id: "p", name: "P", kind: "command", host: "", port: 0 }))

section("serial")
print("normalizeSerialSettings string baud", serial.normalizeSerialSettings({ port: " COM3 ", baudRate: "9600" }))
print("normalizeSerialSettings fractional baud", serial.normalizeSerialSettings({ baudRate: 115200.5 }))
print("normalizeSerialSettings usb", serial.normalizeSerialSettings({ usb: { vid: 1, pid: 2, serialNumber: "" } }))
print("serialSummary", serial.serialSummary({ ...serial.DEFAULT_SERIAL_SETTINGS, baudRate: 921600, parity: "even" }))
print("formatUsbId", serial.formatUsbId({ vid: 0x1a86, pid: 0x7523 }))
print("localEchoText", ["ls\r", "a\r\nb", "\x7f", "\x1b[I", "\u{1F600}\r"].map(serial.localEchoText))
const now = new Date(2026, 0, 1, 9, 5, 7, 42)
const stamp = serial.createLineTimestamper()
print("timestamper chunks", [stamp("boot", now), stamp("ing\r\nready\r\n", now), stamp("> ", now)])
print("sentEchoLine text", serial.sentEchoLine({ text: "AT\r\x00" }))
print("sentEchoLine bytes", serial.sentEchoLine({ bytes: [0xaa, 0x5, 0x0d] }, now))
print("parseHexBytes", ["48 65 6C", "0x48 0X65", "48656c", "48 656", "", "48,; 6c"].map(serial.parseHexBytes))
print("lineEndingBytes", ["cr", "lf", "crlf"].map(serial.lineEndingBytes))

section("sftpStartDirectory and monitorMetrics")
print("sftpStartPath", [
  sftpStartDirectory.sftpStartPath("terminal", true, "/var/log"),
  sftpStartDirectory.sftpStartPath("home", true, "/var/log"),
  sftpStartDirectory.sftpStartPath("home", false, "/var/log"),
  sftpStartDirectory.sftpStartPath("last", false, "/var/log"),
])
print("normalizeMonitorVisibleMetrics", [
  monitorMetrics.normalizeMonitorVisibleMetrics(["uptime", "cpu", "bogus", "cpu"]),
  monitorMetrics.normalizeMonitorVisibleMetrics(["bogus"]),
])

section("optionSearch")
const groupsForSearch = [
  { label: null, options: [{ value: "", label: "Same as the app theme" }] },
  { label: "Presets", options: [{ value: "default", label: "Default" }, { value: "ocean", label: "Ocean" }] },
]
print(
  "filterOptionGroups('night TOMORROW')",
  optionSearch.filterOptionGroups(
    [
      ...groupsForSearch,
      { label: "Library", options: [{ value: "catalog:Tomorrow Night", label: "Tomorrow Night" }] },
    ],
    "night TOMORROW"
  )
)
print("filterOptionGroups('  ')", optionSearch.filterOptionGroups(groupsForSearch, "  "))

section("terminalLogReplay")
function encode(frames) {
  const parts = []
  const push64 = (value) => {
    const view = new DataView(new ArrayBuffer(8))
    view.setBigUint64(0, BigInt(value), true)
    parts.push(...new Uint8Array(view.buffer))
  }
  for (const frame of frames) {
    if (frame.data !== undefined) {
      const bytes = new TextEncoder().encode(frame.data)
      parts.push(0)
      push64(frame.atUs)
      const length = new DataView(new ArrayBuffer(4))
      length.setUint32(0, bytes.length, true)
      parts.push(...new Uint8Array(length.buffer), ...bytes)
    } else {
      parts.push(1)
      push64(frame.atUs)
      const size = new DataView(new ArrayBuffer(4))
      size.setUint16(0, frame.cols ?? 0, true)
      size.setUint16(2, frame.rows ?? 0, true)
      parts.push(...new Uint8Array(size.buffer))
    }
  }
  return new Uint8Array(parts)
}
const encoded = encode([{ atUs: 0, cols: 100, rows: 30 }, { atUs: 1500, data: "hi" }, { atUs: 2500, cols: 120, rows: 40 }])
print("parseReplayFrames kinds", terminalLogReplay.parseReplayFrames(encoded.buffer).map((frame) => ("data" in frame ? "output" : "resize")))
print("parseReplayFrames truncated by 2", terminalLogReplay.parseReplayFrames(encoded.buffer.slice(0, encoded.byteLength - 2)).length)
const badKind = encoded.slice()
badKind[0] = 7
print("parseReplayFrames unknown kind", terminalLogReplay.parseReplayFrames(badKind.buffer).length)
print("initialReplaySize no resize", terminalLogReplay.initialReplaySize([{ atMs: 0, data: new Uint8Array() }]))
const timelineFrames = [1000, 1200, 61200].map((atMs) => ({ atMs, data: new Uint8Array() }))
print("buildReplayTimeline", [
  terminalLogReplay.buildReplayTimeline(timelineFrames, null),
  terminalLogReplay.buildReplayTimeline(timelineFrames, 2000),
])
print("buildReplayTimeline out of order", terminalLogReplay.buildReplayTimeline([{ atMs: 500, data: new Uint8Array() }, { atMs: 100, data: new Uint8Array() }], null))
print("framesPlayedBy", [-1, 0, 1000, 5000].map((time) => terminalLogReplay.framesPlayedBy([0, 200, 2200], time)))
print("formatReplayTime", [0, 65_000, 3_725_000, -1, 59_999].map(terminalLogReplay.formatReplayTime))
print("fitReplayFontSize", [
  terminalLogReplay.fitReplayFontSize(12, 800, 10, 1610),
  terminalLogReplay.fitReplayFontSize(12, 800, 0, 1010),
  terminalLogReplay.fitReplayFontSize(12, 800, 0, 600),
  terminalLogReplay.fitReplayFontSize(12, 800, 10, 1e5),
  terminalLogReplay.fitReplayFontSize(12, 0, 10, 500),
])
print(
  "replayFontSize",
  terminalLogReplay.replayFontSize(12, 16, false, { width: 800, height: 400 }, { width: 14, height: 0 }, { width: 2000, height: 450 })
)

section("terminalCd and droppedPaths")
print("terminalCdCommand", [
  terminalCd.terminalCdCommand("/var/log"),
  terminalCd.terminalCdCommand("/srv/my app/$HOME"),
  terminalCd.terminalCdCommand("/tmp/it's"),
])
print("dropQuotingFor", [
  droppedPaths.dropQuotingFor("windows", "Git Bash"),
  droppedPaths.dropQuotingFor("windows", "nu"),
  droppedPaths.dropQuotingFor("linux", undefined),
])
print("quoteDroppedPath posix", [
  droppedPaths.quoteDroppedPath("/Users/me/My File (1).txt", "posix"),
  droppedPaths.quoteDroppedPath("/tmp/a\nb", "posix"),
  droppedPaths.quoteDroppedPath("/Users/me/文档/报告.pdf", "posix"),
])
print("quoteDroppedPath windows", [
  droppedPaths.quoteDroppedPath("C:\\Program Files\\app", "powershell"),
  droppedPaths.quoteDroppedPath("C:\\tmp\\$it's", "powershell"),
  droppedPaths.quoteDroppedPath("C:\\a&b", "cmd"),
])
print("quoteDroppedPath unix paths", [
  droppedPaths.quoteDroppedPath("D:\\", "wsl"),
  droppedPaths.quoteDroppedPath("C:", "git-bash"),
  droppedPaths.quoteDroppedPath("\\\\wsl$\\Ubuntu", "wsl"),
  droppedPaths.quoteDroppedPath("\\\\server\\share\\it's", "wsl"),
])
print("droppedPathsText", [
  droppedPaths.droppedPathsText(["/a b", "/c"], "posix"),
  droppedPaths.droppedPathsText([], "posix"),
  droppedPaths.droppedPathsText([""], "cmd"),
])

section("externalEditor, notificationBadge, vaultLock, userVerification")
print("editorDisplayName", [
  "/Applications/Visual Studio Code.app",
  "/Applications/Sublime Text.app/",
  "C:\\Program Files\\Notepad++\\notepad++.exe",
  "C:\\Users\\me\\bin\\code.CMD",
  "C:\\",
  " kate ",
  "",
].map(externalEditor.editorDisplayName))
print("badgeLabel", [0, 1, 9, 10, -1].map(notificationBadge.badgeLabel))
print("badgeOverlaySize", [0.5, 1, 1.4, 1.5, 2].map(notificationBadge.badgeOverlaySize))
print("isVaultLocked", [
  vaultLock.isVaultLocked("Saved passwords are locked. Unlock them first."),
  vaultLock.isVaultLocked(new Error("Saved passwords are locked. Unlock them first.")),
  vaultLock.isVaultLocked("Incorrect master password."),
])
print("vaultUnlockable", [
  vaultLock.vaultUnlockable({ storageMode: "password", hasMasterPassword: true, migrationPending: false }),
  vaultLock.vaultUnlockable({ storageMode: "system", hasMasterPassword: false, migrationPending: true }),
  vaultLock.vaultUnlockable({ storageMode: "memory", hasMasterPassword: true, migrationPending: false }),
])
print("verificationRequired", [
  "USER_VERIFICATION_REQUIRED:system",
  "USER_VERIFICATION_REQUIRED:masterPassword",
  "USER_VERIFICATION_REQUIRED:none",
  "USER_VERIFICATION_REQUIRED:system extra",
  "Saved passwords are locked. Unlock them first.",
].map(userVerification.verificationRequired))
print("normalizeVerificationMethod", ["system", "masterPassword", "hello", undefined].map(userVerification.normalizeVerificationMethod))

section("settingsSearch")
const en = JSON.parse(fs.readFileSync("src/i18n/locales/en.json", "utf8"))
const lookup = (messages, key) =>
  key.split(".").reduce((node, part) => (node === undefined ? undefined : node[part]), messages)
const translator = (messages) => (key, options) => lookup(messages, key) ?? options?.defaultValue ?? key
const items = settingsSearch.buildSettingsSearchItems(translator(en))
print("settings items", items.length)
print("searchSettings('FONT SIZE')", settingsSearch.searchSettings(items, "FONT SIZE").slice(0, 3).map((item) => item.title))
print("searchSettings('updates')", settingsSearch.searchSettings(items, "updates").slice(0, 4).map((item) => `${item.title}${item.isPage ? " (page)" : ""}`))
print("searchSettings('terminal font') has Font Size", settingsSearch.searchSettings(items, "terminal font").some((item) => item.title === "Font Size"))
print("searchSettings('   ')", settingsSearch.searchSettings(items, "   ").length)
print("searchSettings('ｆｏｎｔ')", settingsSearch.searchSettings(items, "ｆｏｎｔ").slice(0, 2).map((item) => item.title))

section("SftpDrawer: sftpView")
const entry = (name, overrides = {}) => ({ name, path: `/srv/${name}`, isDir: false, isSymlink: false, ...overrides })
const entries = [
  entry("notes.txt", { size: 300, modifiedAt: 3 }),
  entry("logs", { isDir: true, modifiedAt: 9 }),
  entry("file10.log", { size: 20, modifiedAt: 1 }),
  entry(".env", { size: 5, modifiedAt: 2 }),
  entry("File2.log", { size: 100, modifiedAt: 4 }),
  entry("app", { isDir: true, modifiedAt: 5 }),
  entry("file9.log", { size: 7, modifiedAt: 6 }),
]
for (const sort of [
  { column: "name", direction: "asc" },
  { column: "name", direction: "desc" },
  { column: "size", direction: "desc" },
  { column: "modified", direction: "asc" },
  { column: "kind", direction: "asc" },
  { column: "owner", direction: "asc" },
  { column: "permissions", direction: "asc" },
]) {
  print(`sortSftpEntries ${sort.column} ${sort.direction}`, sftpView.sortSftpEntries(entries, sort).map((item) => item.name))
}
print("parseOctalMode", ["644", " 2755 ", "64", "688", "07777 ", "rwx"].map(sftpView.parseOctalMode))
print("formatOctalMode", [0o644, 0o7, 0o4755, -1].map(sftpView.formatOctalMode))

section("SftpDrawer: sftpHistory")
let history = sftpHistory.EMPTY_SFTP_HISTORY
for (const path of Array.from({ length: sftpHistory.SFTP_HISTORY_LIMIT + 5 }, (_, index) => `/d${index}`)) {
  history = sftpHistory.recordSftpVisit(history, path)
}
print("history over the limit", [history.paths.length, history.paths[0], history.index])
print("landSftpHistory resolved path", sftpHistory.landSftpHistory(sftpHistory.recordSftpVisit({ paths: ["/link", "/var"], index: 1 }, "/var"), 0, "/real"))
print("dropSftpHistoryEntry current", sftpHistory.dropSftpHistoryEntry({ paths: ["/a", "/b"], index: 1 }, 1))

section("SftpDrawer: sftpArchive")
print("archiveFormatOf", [".zip", "a.zip", "site.TAR.GZ", "a.tgz", "a.tar", "notes.txt"].map((name) => sftpArchive.archiveFormatOf({ name, isDir: false })))
print("defaultExtractDestination", [
  sftpArchive.defaultExtractDestination("/srv/site.tar.gz", "tarGz"),
  sftpArchive.defaultExtractDestination("/dump.xz", "xz"),
  sftpArchive.defaultExtractDestination("/a/b/site.tgz", "tarGz"),
])
print("defaultArchiveName", [
  sftpArchive.defaultArchiveName([{ name: "logs" }], "/srv", "tarGz"),
  sftpArchive.defaultArchiveName([{ name: "a" }, { name: "b" }], "/", "tar"),
  sftpArchive.defaultArchiveName([{ name: "a" }, { name: "b" }], "/srv/", "zip"),
])
print("withArchiveExtension", [
  sftpArchive.withArchiveExtension("logs.tar.gz", "tarGz", "zip"),
  sftpArchive.withArchiveExtension("LOGS.TAR.GZ", "tarGz", "zip"),
  sftpArchive.withArchiveExtension("a.tgz", "tarGz", "zip"),
])
print("commonParent", [
  sftpArchive.commonParent([{ path: "/srv/a" }, { path: "/srv/b" }]),
  sftpArchive.commonParent([{ path: "/a" }]),
  sftpArchive.commonParent([]),
])

section("SftpDrawer: sftpSearch")
const matcher = (query, options) => {
  const created = sftpSearch.createSftpSearchMatcher(query, { ...sftpSearch.DEFAULT_SFTP_SEARCH_OPTIONS, ...options })
  return {
    hasQuery: created.hasQuery,
    error: created.error?.message ?? null,
    hits: ["README.md", "readme.txt", "a.log", "A.LOG", "b1", "档案.txt"].filter((name) =>
      created.matches({ name, path: `/srv/${name}`, isDir: false, isSymlink: false })
    ),
  }
}
print("matcher plain", matcher("read", {}))
print("matcher glob", matcher("*.log", {}))
print("matcher negated class", matcher("[!a]*", {}))
print("matcher regex", matcher("^a\\.log$", { regex: true }))
print("matcher invalid regex with glob", matcher("a[", { regex: true }))
print("matcher invalid regex", matcher("a(", { regex: true }))
print("matcher empty", matcher("   ", {}))
print("MATCH_ALL", sftpSearch.MATCH_ALL.matches({ name: "x" }))

section("SftpDrawer: sftpDrawerUtils")
print("joinRemotePath", [["/", "a"], ["/srv", "a"], ["/srv/", "a"]].map(([base, name]) => drawerUtils.joinRemotePath(base, name)))
print("parentRemotePath", ["/a/b", "/a", "/", ""].map(drawerUtils.parentRemotePath))
print("isPermissionDenied", ["Permission denied", "permission denied (SSH_FX_PERMISSION_DENIED)", null].map(drawerUtils.isPermissionDenied))
print("formatBytes", [undefined, 0, 1023, 1024, 1280, 1536, 10240, 1024 * 1024 - 1, 5 * 1024 * 1024, 1024 ** 4].map(drawerUtils.formatBytes))
print("formatTimestamp", [undefined, 0, Date.UTC(2026, 0, 1, 9, 5, 7, 42)].map(drawerUtils.formatTimestamp))
print("MAX_EDIT_FILE_BYTES", drawerUtils.MAX_EDIT_FILE_BYTES)

section("sync")
const sync = await load("@/lib/sync")
print("SYNC events", [sync.SYNC_SETTINGS_EVENT, sync.SYNC_APPLIED_EVENT, sync.SYNC_STATUS_EVENT])
print("intervals", [sync.SYNC_PUSH_INTERVALS_SECS, sync.SYNC_PULL_INTERVALS_MINS])

// The updater reads import.meta.env at module scope and is covered by its own
// vitest suite with Tauri mocks, so this oracle leaves it to that suite.
