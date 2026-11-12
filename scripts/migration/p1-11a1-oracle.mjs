// Runs the frontend's terminal-output helpers without a browser or changes to
// src/. The output is the source of every expected value in the Rust tests
// that the TypeScript tests do not already state.
import fs from "node:fs"
import ts from "typescript"

function moduleUrl(path, imports = {}) {
  let source = ts.transpileModule(fs.readFileSync(path, "utf8"), {
    compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ESNext },
  }).outputText
  for (const [specifier, url] of Object.entries(imports)) {
    source = source.replaceAll(`"${specifier}"`, JSON.stringify(url))
  }
  return `data:text/javascript;base64,${Buffer.from(source).toString("base64")}`
}

const reactStub = `data:text/javascript;base64,${Buffer.from(
  "export function useSyncExternalStore() { return undefined }"
).toString("base64")}`

const terminalNotificationsUrl = moduleUrl("src/lib/terminalNotifications.ts")
const agentResumeCwdUrl = moduleUrl("src/lib/agentResumeCwd.ts")
const agentStatusUrl = moduleUrl("src/lib/agentStatus.ts", {
  "@/lib/agentResumeCwd": agentResumeCwdUrl,
})
const osc52Url = moduleUrl("src/lib/osc52.ts", {
  "@/lib/terminalNotifications": terminalNotificationsUrl,
})
const terminalCwdUrl = moduleUrl("src/lib/terminalCwd.ts")
const remoteCwdUrl = moduleUrl("src/lib/remoteCwd.ts", { react: reactStub })
const terminalCommandCaptureUrl = moduleUrl("src/lib/terminalCommandCapture.ts")
const terminalTabUtilsUrl = moduleUrl("src/components/TerminalTab/terminalTabUtils.ts")
const terminalOutputScannerUrl = moduleUrl("src/lib/terminalOutputScanner.ts", {
  "@/components/TerminalTab/terminalTabUtils": terminalTabUtilsUrl,
})
const sudoPromptUrl = moduleUrl("src/lib/sudoPrompt.ts")
const passwordFeedbackUrl = moduleUrl("src/lib/passwordFeedback.ts")
const loginScriptUrl = moduleUrl("src/lib/loginScript.ts")
const pasteGuardUrl = moduleUrl("src/lib/pasteGuard.ts")
const distroUrl = moduleUrl("src/lib/distro.ts")

const notifications = await import(terminalNotificationsUrl)
const agents = await import(agentStatusUrl)
const agentResume = await import(agentResumeCwdUrl)
const osc52 = await import(osc52Url)
const terminalCwd = await import(terminalCwdUrl)
const remoteCwd = await import(remoteCwdUrl)
const commandCapture = await import(terminalCommandCaptureUrl)
const terminalTabUtils = await import(terminalTabUtilsUrl)
const outputScanner = await import(terminalOutputScannerUrl)
const sudoPrompt = await import(sudoPromptUrl)
const passwordFeedback = await import(passwordFeedbackUrl)
const loginScript = await import(loginScriptUrl)
const pasteGuard = await import(pasteGuardUrl)
const distro = await import(distroUrl)

function encoded(text) {
  return btoa(String.fromCharCode(...new TextEncoder().encode(text)))
}

function attempt(action) {
  try {
    return { value: action() }
  } catch (error) {
    return { error: `${error.name}: ${error.message}` }
  }
}

const jsText = {
  lengths: ["", "abc", "a😀中文", "e\u0301"].map((text) => ({
    text,
    length: text.length,
    units: [...text].map((character) => character.codePointAt(0)),
  })),
  slices: [
    ["a😀bc", 0, 3],
    ["a😀bc", 1, 3],
    ["a😀bc", 0, 2],
    ["a😀bc", 2],
  ].map(([text, start, end]) => ({
    text,
    start,
    end: end ?? null,
    result: end === undefined ? text.slice(start) : text.slice(start, end),
  })),
  tails: [
    ["abc", 0],
    ["abc", 2],
    ["a😀", 2],
    ["a😀bc", 4],
  ].map(([text, count]) => ({ text, count, result: text.slice(-count) })),
  trims: ["", "  hello  ", "\u3000中文\u00a0", "\ufeffx\ufeff"].map((text) => ({
    text,
    result: text.trim(),
  })),
}

const base64Cases = [
  "",
  " ",
  "\n",
  "YQ==",
  "YQ",
  "YQ=",
  "ab",
  "ab=",
  "ab==",
  "a",
  "abc=",
  "ab+c",
  "ab-c",
  "ab_c",
  "ab!",
  "ab===",
  "====",
  "Y Q==",
  "YQ==\n",
  "AA==",
  "/w==",
  "77u/YQ==",
  "77u/",
  "77u/77u/YQ==",
  "5L2g5aW9",
].map((text) => ({ text, ...attempt(() => notifications.decodeBase64Utf8(text)) }))

const uriCases = [
  "%C3%A9",
  "%E4%B8%AD",
  "%F0%9F%98%80",
  "a%20b",
  "%2F",
  "100%",
  "%",
  "%2",
  "%GG",
  "%C3",
  "%ED%A0%80",
  "x%41y",
].map((text) => ({ text, ...attempt(() => decodeURIComponent(text)) }))

const osc52Cases = [
  `c;${encoded("hello world")}`,
  `c;${encoded("你好，vim\n")}`,
  `;${encoded("a")}`,
  `p;${encoded("a")}`,
  `s0;${encoded("a")}`,
  "c;?",
  "c;",
  "aGVsbG8=",
  "x;aGVsbG8=",
  "c;not base64!",
  "c;YQ",
  "c;YQ==",
  "c;YQ=",
  "c;AA==",
  "c;5L2g5aW9",
].map((data) => ({ data, result: osc52.parseOsc52Copy(data) }))

const cwdCases = [
  [7, "file:///home/me/project"],
  [7, "file:///c/Users/me"],
  [7, "file://localhost/tmp"],
  [7, "file:///home/me/a b%2520中"],
  [7, "file:///home/me/caf%C3%A9"],
  [7, "file:///home/me/100%"],
  [7, "file:///C:/Users/me"],
  [7, "file:///D:"],
  [7, "file://build-server/home/me"],
  [7, "/home/me"],
  [7, "file://"],
  [7, "file:///tmp\n"],
  [7, "file://Web01/srv/my%20app"],
  [9, '9;"C:\\Users\\me\\My Projects"'],
  [9, "9;C:\\Windows"],
  [9, "9;\\\\server\\share\\dir"],
  [9, "4;1;50"],
  [9, "Build finished"],
  [9, '9;""'],
  [9, "9;\n"],
  [133, "A"],
].map(([osc, data]) => ({
  osc,
  data,
  result: terminalCwd.parseCwdReport(osc, data),
  parsed: osc === 7 ? terminalCwd.parseOsc7Report(data) : null,
}))

const hostCases = [
  ["", true],
  ["LocalHost", true],
  ["web01", false],
  ["web01", "WEB01.example.com"],
  ["web01.a.com", "web01.b.com"],
  ["web01", "web02"],
].map(([first, second]) => ({ first, second, result: typeof second === "boolean" ? terminalCwd.isUnnamedHost(first) : terminalCwd.isSameHost(first, second) }))

remoteCwd.setTabRemoteCwd("tab-a", "/srv")
remoteCwd.setTabRemoteCwd("tab-b", "/tmp")
remoteCwd.setTabRemoteCwd("tab-a", "/var/log")
const remoteBefore = {
  a: remoteCwd.getTabRemoteCwd("tab-a"),
  b: remoteCwd.getTabRemoteCwd("tab-b"),
}
remoteCwd.setTabRemoteHostname("tab-a", "web01")
remoteCwd.setTabRemoteAwayHost("tab-a", "db01")
const remoteAfter = {
  away: remoteCwd.getTabRemoteAwayHost("tab-a"),
  cwd: remoteCwd.getTabRemoteCwd("tab-a"),
  hostname: remoteCwd.getTabRemoteHostname("tab-a"),
}
remoteCwd.setTabRemoteHostname("tab-a", null)
remoteCwd.setTabRemoteAwayHost("tab-a", null)
remoteCwd.setTabRemoteCwd("tab-a", null)
remoteCwd.setTabRemoteCwd("tab-b", null)
const remoteCleared = {
  away: remoteCwd.getTabRemoteAwayHost("tab-a") ?? null,
  cwd: remoteCwd.getTabRemoteCwd("tab-a") ?? null,
  hostname: remoteCwd.getTabRemoteHostname("tab-a") ?? null,
}

const commandCases = [
  ["git status", "\r"],
  ["密码🔑", "\r"],
  ["git sttus", "\x1b[D\x1b[D\x1b[D", "a", "\r"],
  ["\x1b[200~printf 'a'\nprintf 'b'\x1b[201~", "\r"],
  ["\x1b[A\r"],
  ["\x1b", "[A\r"],
  ["\x1b[200", "~abc\x1b[201~", "\r"],
  ["git add 中文文件", "\x17", "\r"],
  ["a😀 b", "\x17", "\r"],
  ["git  -s", "\x1b[D\x1b[D\x1b[D", "\x1b[3~", "\r"],
  ["\u0085", "\r"],
].map((steps) => {
  let state = commandCapture.EMPTY_COMMAND_CAPTURE_STATE
  const commands = []
  for (const step of steps) {
    const result = commandCapture.captureTerminalInput(state, step)
    state = result.state
    commands.push(...result.commands)
  }
  return { steps, state, commands }
})
const shellIntegrationCases = [
  "E;docker ps",
  "E;printf a; printf b",
  "D;0",
  "E;dir C:\\x5ctmp\\x0a\\x09ls \\\\x",
  "E;\\x20 ",
  "C;dir C:\\x5ctmp",
  "C;  ls  ",
  "E;\\xZZ",
].map((data) => ({ data, result: commandCapture.parseShellIntegrationCommand(data) }))

const scanCases = [
  ["", "\r\n\x1b[33m[Connecting to jump host #1 user@host:22]\x1b[0m\r\n"],
  ["\r\n\x1b[33m[Conn", "ecting to user@host:22]\x1b[0m\r\n"],
  ["$ echo hi\nhi\n", "x".repeat(1000)],
  ["😀".repeat(8), "x"],
].map(([first, second]) => {
  const one = outputScanner.scanTerminalOutput(outputScanner.EMPTY_OUTPUT_SCAN_STATE, first)
  const two = outputScanner.scanTerminalOutput(one.state, second)
  return { first, second, one, two }
})
const decodeCases = [
  [0x68, 0xc3, 0xa9],
  [0xe4, 0xb8],
  [0x61, 0xe4, 0xb8, 0x62],
  [0xf0, 0x9f],
  [0xf0, 0x9f, 0x92],
  [0xe0, 0x80, 0x80],
  [0xed, 0xa0, 0x80],
  [0xf4, 0x90, 0x80, 0x80],
  [0xc0, 0xaf],
  [0xef, 0xbb, 0xbf, 0x61],
  [0xef, 0xbb, 0xbf],
  [0xef, 0xbb, 0xbf, 0xef, 0xbb, 0xbf],
  [0x61, 0xef, 0xbb, 0xbf],
].map((bytes) => ({
  bytes,
  result: outputScanner.decodeOutputChunk(new Uint8Array(bytes)),
}))

const promptCases = [
  ["[sudo] password for admin: ", "admin", null],
  ["[sudo] password for 'admin': ", "admin", null],
  ["[sudo: authenticate] Password: ", "stone", null],
  ["[sudo: authenticate] 密码：", "stone", null],
  ["[sudo: authenticate] PIN: ", "stone", null],
  ["[sudo: authenticate] Verification code: ", "stone", null],
  ["[sudo] stone 的密码：", null, null],
  ["[sudo] stone 的密碼：", null, null],
  ["[sudo] Passwort für stone: ", "stone", null],
  ["[sudo] mot de passe de stone : ", "stone", null],
  ["[sudo] Passwort für root: ", "stone", null],
  ["[sudo] Passwort für stoneage: ", "stone", null],
  ["doas (ops@web-1) password: ", "ops", null],
  ["[sudo] password for root: ", "stone", null],
  ["stone@db-2's password: ", "stone", null],
  ["stone@host:~$ ", "stone", null],
  ["my password for fun: 123", "stone", null],
  ["Password:", "stone", null],
].map(([line, user]) => ({ line, user, result: sudoPrompt.matchPasswordPrompt(line, user ?? undefined, []) }))
const promptBoundaryCases = [
  "[sudo: authenticate] PASSWORD:",
  "[sudo: authenticate] Passwort:",
  "[sudo: authenticate] contrase\u00f1a:",
  "[sudo: authenticate] ПАРОЛЬ：",
  `[sudo] ${"a".repeat(244)} 的密码：`,
  `[sudo] ${"a".repeat(245)} 的密码：`,
  "[sudo] Password für a.b: ",
].map((line) => ({
  line,
  result: sudoPrompt.matchPasswordPrompt(line, "stone", []),
}))
const sudoEmptyKnownUser = {
  empty: sudoPrompt.matchPasswordPrompt("[sudo] mot de passe de root : ", "", []),
  missing: sudoPrompt.matchPasswordPrompt("[sudo] mot de passe de root : ", undefined, []),
  whitespace: sudoPrompt.matchPasswordPrompt("[sudo] mot de passe de   root : ", " ", []),
}
const sudoEmojiBoundary = [122, 123].map((count) => {
  const line = `[sudo] ${"😀".repeat(count)} 的密码：`
  return {
    count,
    charLength: line.length,
    result: sudoPrompt.matchPasswordPrompt(line, "stone", []),
  }
})
const customPatterns = sudoPrompt.compilePromptPatterns([
  "^Password for (?<user>\\S+) on \\S+:$",
  "^Password:$",
  "(",
])
const customCases = [
  "Password for ops on db:",
  "Password:",
  "Enter PIN>",
].map((line) => ({ line, result: sudoPrompt.matchPasswordPrompt(line, "ops", customPatterns) }))
const validPatterns = ["^Password:$", "(", "", "\\"].map((pattern) => ({
  pattern,
  result: sudoPrompt.isValidPromptPattern(pattern),
}))
const trimmedPatterns = sudoPrompt.compilePromptPatterns([
  "  ^Password:$  ",
  "\u3000^PIN:$\u3000",
  "   ",
  "(",
])

const loginParses = ["cd /srv/app\r\n\n  \rsudo -i  \n", "  echo hi", "", undefined, null].map((script) => ({
  script: script ?? null,
  result: loginScript.parseLoginScript(script),
}))
const promptLines = [
  "user@host:~$ ",
  "root@host:~# ",
  "host% ",
  "PS C:\\> ",
  "~/src ❯ ",
  "~/src ➜ ",
  "~/src λ ",
  "~/src » ",
  "[sudo] password for user: ",
  "Last login: Mon Oct  5 10:00:00 2026",
  "",
  "foo$\n",
  "foo$\nbar",
].map((line) => ({ line, result: loginScript.looksLikeShellPrompt(line) }))

const pasteCases = [
  "ls\npwd",
  "rm -rf build\r\n",
  "echo hi\r",
  "git status",
].map((text) => ({
  text,
  enabled: pasteGuard.pasteNeedsConfirmation(text, { enabled: true, bracketedPasteMode: false }),
  bracketed: pasteGuard.pasteNeedsConfirmation(text, { enabled: true, bracketedPasteMode: true }),
  disabled: pasteGuard.pasteNeedsConfirmation(text, { enabled: false, bracketedPasteMode: false }),
}))
const pasteSummaries = [
  "a\r\nb\rc\n",
  "one\n",
  "",
  "😀中文\n第二行",
  "e\u0301".repeat(161),
  `${"x".repeat(pasteGuard.PASTE_PREVIEW_MAX_LINE_LENGTH + 10)}\nok`,
  Array.from({ length: pasteGuard.PASTE_PREVIEW_MAX_LINES + 4 }, (_, index) => `line ${index}`).join("\n"),
].map((text) => ({ text, result: pasteGuard.summarizePaste(text) }))
const passwordCases = [
  [0, "s"],
  [1, "ecret"],
  [0, "密码🔑"],
  [3, "\x7f"],
  [3, "\b"],
  [0, "\x7f"],
  [5, "\x15ab"],
  [2, "\t\x04"],
  [4, "\r"],
  [4, "pw\n"],
  [4, "\x03"],
  [3, ""],
  [-1, "a"],
  [0, "e\u0301"],
].map(([typed, data]) => ({ typed, data, result: passwordFeedback.nextTypedLength(typed, data) }))

const distroInfos = [
  { id: "ubuntu", idLike: ["debian"], name: "", prettyName: "" },
  { id: "amzn", idLike: [], name: "", prettyName: "" },
  { id: "linuxmint", idLike: ["ubuntu", "debian"], name: "", prettyName: "" },
  { id: "manjaro", idLike: ["arch"], name: "", prettyName: "" },
  { id: "someos", idLike: ["rhel", "fedora"], name: "", prettyName: "" },
  { id: "freebsd", idLike: [], name: "", prettyName: "FreeBSD 14.1-RELEASE" },
  { id: "macos", idLike: [], name: "macOS", prettyName: "" },
  { id: "someos", idLike: [], name: "Some OS", prettyName: "" },
  undefined,
  { id: "mx", idLike: [], name: "", prettyName: "" },
  { id: "someos", idLike: [], name: "MX Linux", prettyName: "" },
  { id: "someos", idLike: [], name: "Linux Mint", prettyName: "" },
  { id: "someos", idLike: [], name: "Arch Linux", prettyName: "" },
].map((info) => ({ info: info ?? null, result: distro.detectDistroId(info) }))
const distroReports = [
  "tterm-os;ubuntu;debian;Ubuntu",
  "tterm-os;rocky;rhel centos fedora;Rocky Linux",
  "tterm-os;pop;ubuntu debian;Pop!_OS",
  "tterm-agent;claude;done",
  "notify;title;body",
  "tterm-os;;;",
  "tterm-os;ubuntu;debian;Ubuntu;extra",
  "tterm-os;;;name",
  "tterm-os",
].map((data) => ({ data, result: distro.parseOsReport(data) }))
const distroTable = distro.DISTRO_IDS.map((id) => ({
  id,
  name: distro.distroName(id),
  icon: distro.distroIconSrc(id),
  valid: distro.isDistroId(id),
}))

const notificationCases = [
  ["parse9", "Claude is waiting for your input"],
  ["parse9", "9;C:\\Users"],
  ["parse9", "4;1;50"],
  ["parse9", "4"],
  ["parse9", "9"],
  ["parse9", "9\n"],
  ["parse9", "9\r\n"],
  ["parse9", "a\x07b\x1b"],
  ["parse9", "  "],
  ["parse9", "x".repeat(5000)],
  ["parse777", "notify;Build;done; 3 warnings"],
  ["parse777", "notify;Build;"],
  ["parse777", "precmd"],
].map(([kind, data]) => ({
  kind,
  data,
  result:
    kind === "parse9"
      ? notifications.parseOsc9Notification(data)
      : notifications.parseOsc777Notification(data),
}))
const notificationTruncation = [
  "😀".repeat(600),
  "e\u0301".repeat(600),
].map((data) => ({ data, result: notifications.parseOsc9Notification(data) }))

function kittySteps(steps) {
  const kitty = new notifications.KittyNotificationAssembler()
  return steps.map((step) => ({ step, result: kitty.handle(step) }))
}
const kittyCases = [
  kittySteps([";Hello"]),
  kittySteps(["i=1:d=0;Build", "i=1:p=body;finished"]),
  kittySteps([`e=1;${encoded("完成")}`]),
  kittySteps(["i=1:p=?;", "p=close;", "no separator"]),
  kittySteps(["i=1:p=title;one", "i=1:d=0:p=body;two", "i=1:p=body;three"]),
  kittySteps(["i=1:t=one:p=body;body", "i=1:p=body;more"]),
  kittySteps(["i=1:p=title:a", "p=title:b"]),
  kittySteps(["i=1:d=0:p=title;one", "i=1:d=0:p=title;two"]),
  kittySteps(["i=1:p=title:x", "i=1:d=0:p=body;y", "i=1:p=body;z"]),
  kittySteps([
    "i=0:d=0:p=title;zero",
    ...Array.from({ length: 16 }, (_, index) => `i=${index + 1}:d=0:p=title;t${index + 1}`),
    "i=0:p=body;late",
  ]),
]

function timerSteps(steps) {
  const running = []
  const timer = new notifications.CommandTimer((command) => running.push(command))
  const results = []
  for (const step of steps) {
    if (step[0] === "prompt") results.push(timer.prompt() ?? null)
    else if (step[0] === "start") results.push(timer.start(step[1]) ?? null)
    else if (step[0] === "command") results.push(timer.setCommand(step[1]) ?? null)
    else if (step[0] === "typed") results.push(timer.setTypedCommand(step[1]) ?? null)
    else if (step[0] === "mark") results.push(timer.mark(step[1], step[2]) ?? null)
    else if (step[0] === "finish") results.push(timer.finish(step[1], step[2]) ?? null)
  }
  return { results, running }
}
const timerCases = [
  timerSteps([["prompt"], ["command", "make"], ["start", 1000], ["finish", 0, 13000]]),
  timerSteps([["start", 1000], ["start", 1500], ["finish", 1, 2000]]),
  timerSteps([["finish", 0, 1000], ["start", 1000], ["prompt"], ["finish", 0, 2000]]),
  timerSteps([["command", "make"], ["start", 0], ["finish", 0, 10], ["start", 20], ["finish", 0, 30]]),
  timerSteps([
    ["typed", "ls"],
    ["mark", { kind: "prompt" }, 0],
    ["typed", "python"],
    ["start", 10],
    ["mark", { kind: "output" }, 20],
    ["mark", { kind: "done", exitCode: 0 }, 30],
    ["mark", { kind: "prompt" }, 40],
  ]),
  timerSteps([["mark", { kind: "prompt" }, 0], ["start", 10], ["typed", "fix the login bug"], ["finish", 0, 30]]),
  timerSteps([
    ["mark", { kind: "prompt" }, 0],
    ["typed", "cl"],
    ["start", 10],
    ["command", "claude --resume"],
    ["command", "claude --resume"],
    ["finish", 0, 30],
  ]),
]

const settings = {
  notifications_enabled: true,
  notify_command_finished: true,
  notify_command_min_secs: 10,
  notify_terminal_requests: true,
  notify_agent_waiting: true,
  notify_agent_done: true,
  bell_notify: false,
}
const attentionCases = [
  [{ kind: "request", body: "hi" }, settings, { windowFocused: true, tabVisible: true, tabFocused: false }],
  [{ kind: "bell" }, settings, { windowFocused: true, tabVisible: true, tabFocused: false }],
  [{ kind: "command", exitCode: 0, durationMs: 30000 }, settings, { windowFocused: true, tabVisible: true, tabFocused: true }],
  [{ kind: "command", exitCode: 0, durationMs: 30000 }, settings, { windowFocused: false, tabVisible: true, tabFocused: true }],
  [{ kind: "agent", agent: "claude", state: "waiting" }, settings, { windowFocused: false, tabVisible: false, tabFocused: false }],
  [{ kind: "agent", agent: "claude", state: "done" }, settings, { windowFocused: false, tabVisible: false, tabFocused: false }],
  [{ kind: "agent", agent: "claude", state: "error" }, settings, { windowFocused: false, tabVisible: false, tabFocused: false }],
  [{ kind: "bell" }, { ...settings, bell_notify: true }, { windowFocused: false, tabVisible: false, tabFocused: false }],
  [{ kind: "bell" }, { ...settings, bell_notify: true }, { windowFocused: true, tabVisible: false, tabFocused: false }],
  [{ kind: "request", body: "hi" }, { ...settings, notifications_enabled: false }, { windowFocused: false, tabVisible: false, tabFocused: false }],
].map(([event, current, focus]) => ({ event, settings: current, focus, result: notifications.decideAttention(event, current, focus) }))
const durations = [0, 499, 499.99999999999994, 500, 42400, 42500, 59999, 60500, 185000, 3599500, 3720000, -500, -1500, NaN, Infinity, -Infinity].map((value) => ({
  value: Number.isNaN(value) ? "NaN" : value === Infinity ? "Infinity" : value === -Infinity ? "-Infinity" : value,
  result: notifications.formatCommandDuration(value),
}))
const stronger = [
  [undefined, "activity"],
  ["activity", "bell"],
  ["notification", "activity"],
  ["bell", "notification"],
].map(([current, next]) => ({ current: current ?? null, next, result: notifications.strongerAttention(current, next) }))
const localMark = [
  [{ command_marks: true, local_unix_shell_integration: true }, "macos"],
  [{ command_marks: true, local_unix_shell_integration: false }, "linux"],
  [{ command_marks: true, local_unix_shell_integration: false }, "windows"],
  [{ command_marks: false, local_unix_shell_integration: true }, "windows"],
].map(([config, platform]) => ({ config, platform, result: notifications.localShellsMarkCommands(config, platform) }))

const agentParses = [
  "tterm-agent;claude;processing",
  "tterm-agent;my-agent2;ended",
  "tterm-agent;codex;done;01a12490-4b68-76d0",
  "tterm-agent;codex;done;abc-123;direct",
  "tterm-agent;codex;done;abc-123;broadcast",
  "tterm-agent;codex;done;abc-123;other",
  "notify;tterm-agent;done",
  "precmd",
  "tterm-agent;claude;processing;abc-123;direct;C%3A%5CJohn%E2%80%99s%20files%3Bx",
  "tterm-agent;codex;done;abc-123;direct;%0A",
  "tterm-agent;claude;done;",
  `tterm-agent;claude;processing;abc-123;direct;${"a".repeat(16384)}`,
  `tterm-agent;claude;processing;abc-123;direct;${"a".repeat(16385)}`,
  "tterm-agent;claude;processing;abc-123;direct;",
].map((data) => ({ data, result: agents.parseAgentReport(data) }))
const displayNames = ["claude", "codex", "opencode", "pi", "aider"].map((agent) => ({
  agent,
  result: agents.agentDisplayName(agent),
}))
const resumeCwd = [
  ["C:/Users/Stone/中文 dir", "c:\\users\\stone\\中文 dir"],
  ["\\\\?\\C:\\work", "c:/work"],
  ["\\\\?\\UNC\\HOST\\Share\\work", "\\\\host\\share\\work"],
  ["/work/中文 dir", "/work/中文 dir"],
  [undefined, "/work"],
  ["/work", undefined],
  ["", ""],
  ["C:/work", "C:/external"],
  ["/Work", "/work"],
  ["/a\\b", "/a/b"],
  ["/work\n", "/work\n"],
].map(([hook, shell]) => ({ hook: hook ?? null, shell: shell ?? null, result: agentResume.agentResumeCwdMatches(hook, shell) }))
const agentCwdBoundaries = ["😀".repeat(8192), "😀".repeat(8193)].map((cwd) => ({
  cwdLength: cwd.length,
  result: agents.parseAgentReport(`tterm-agent;codex;done;abc-123;direct;${cwd}`) !== null,
}))
const cwdEmoji = terminalCwd.parseCwdReport(7, "file:///tmp/%F0%9F%98%80")
const connectingConstants = {
  markerLength: terminalTabUtils.STATUS_CONNECTING.length,
  carryChars: terminalTabUtils.STATUS_CONNECTING.length - 1,
}
const constants = {
  osc52Code: osc52.OSC52_CODE,
  notificationOscCodes: notifications.NOTIFICATION_OSC_CODES,
  cwdReportOscCodes: terminalCwd.CWD_REPORT_OSC_CODES,
  loginPromptIdleMs: loginScript.LOGIN_SCRIPT_PROMPT_IDLE_MS,
  loginFallbackIdleMs: loginScript.LOGIN_SCRIPT_FALLBACK_IDLE_MS,
  pastePreviewMaxLines: pasteGuard.PASTE_PREVIEW_MAX_LINES,
  pastePreviewMaxLineLength: pasteGuard.PASTE_PREVIEW_MAX_LINE_LENGTH,
  agentWaitAnnounceDelayMs: agents.AGENT_WAIT_ANNOUNCE_DELAY_MS,
  agentClaimWindowMs: agents.AGENT_CLAIM_WINDOW_MS,
}
const distroSerialization = distro.parseOsReport("tterm-os;ubuntu;debian;Ubuntu")

console.log(
  JSON.stringify(
    {
      jsText,
      base64Cases,
      uriCases,
      osc52Cases,
      cwdCases,
      hostCases,
      remoteCwd: { remoteBefore, remoteAfter, remoteCleared },
      commandCases,
      shellIntegrationCases,
      scanCases,
      decodeCases,
      promptCases,
      promptBoundaryCases,
      sudoEmptyKnownUser,
      sudoEmojiBoundary,
      customCases,
      validPatterns,
      trimmedPatterns: trimmedPatterns.map((pattern) => pattern.source),
      loginParses,
      promptLines,
      pasteCases,
      pasteSummaries,
      passwordCases,
      distroInfos,
      distroReports,
      distroTable,
      notificationCases,
      notificationTruncation,
      kittyCases,
      timerCases,
      attentionCases,
      durations,
      stronger,
      localMark,
      agentParses,
      displayNames,
      resumeCwd,
      agentCwdBoundaries,
      cwdEmoji,
      connectingConstants,
      constants,
      distroSerialization,
    },
    null,
    2
  )
)
