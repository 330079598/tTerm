// Runs the real TypeScript modules without a browser or changes to src/.
import fs from "node:fs"
import ts from "typescript"

function moduleUrl(path, imports = {}) {
  let source = ts.transpileModule(fs.readFileSync(path, "utf8"), {
    compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ESNext },
  }).outputText
  for (const [specifier, url] of Object.entries(imports)) {
    source = source.replaceAll(JSON.stringify(specifier), JSON.stringify(url))
  }
  return `data:text/javascript;base64,${Buffer.from(source).toString("base64")}`
}

function stubUrl(source) {
  return `data:text/javascript;base64,${Buffer.from(source).toString("base64")}`
}

const commandMarksUrl = moduleUrl("src/components/TerminalTab/commandMarks.ts")
const cursorMoveHoldUrl = moduleUrl("src/components/TerminalTab/cursorMoveHold.ts")
const predictiveEchoUrl = moduleUrl("src/lib/predictiveEcho.ts")
const terminalCommandCaptureUrl = moduleUrl("src/lib/terminalCommandCapture.ts")
const terminalNotificationsUrl = moduleUrl("src/lib/terminalNotifications.ts")
const sudoPromptUrl = moduleUrl("src/lib/sudoPrompt.ts")
const loginScriptUrl = moduleUrl("src/lib/loginScript.ts")
const passwordFeedbackUrl = moduleUrl("src/lib/passwordFeedback.ts")

const marks = await import(commandMarksUrl)
const cursorMoveHold = await import(cursorMoveHoldUrl)
const predictiveEcho = await import(predictiveEchoUrl)
const loginScriptModule = await import(loginScriptUrl)

const markCases = [
  "A",
  "A;cl=m;aid=1",
  "B",
  "C",
  "C;ls -la",
  "D",
  "D;0",
  "D;-0",
  "D;-7",
  "D;x",
  "D;1;2",
  "D;9223372036854775808",
  "E;ls -la",
  "",
].map((data) => ({ data, result: marks.parseCommandMark(data) }))

const markEventTerminal = {
  buffer: { active: { type: "normal" } },
  registerMarker: () => ({
    line: 0,
    isDisposed: false,
    dispose() {},
    onDispose() {},
  }),
  registerDecoration: () => ({ dispose() {}, onRender() {} }),
}
const markEvents = []
const markEventMarks = new marks.CommandMarks(markEventTerminal)
const stopMarkEvents = markEventMarks.onPrompt((event) => markEvents.push(event))
markEventMarks.handleMark("A")
const afterFirstPrompt = [...markEvents]
markEventMarks.setEnabled(false)
const afterDisable = [...markEvents]
markEventMarks.setEnabled(true)
markEventMarks.handleMark("A")
const afterReenable = [...markEvents]
stopMarkEvents()
markEventMarks.handleMark("A")
const afterStop = [...markEvents]
const markPromptEventCases = {
  afterFirstPrompt,
  afterDisable,
  afterReenable,
  afterStop,
}

const line = (text, isWrapped = false) => ({
  isWrapped,
  translateToString: () => text,
})
const joinCases = [
  [],
  [line("")],
  [line("abc"), line("def", true), line("x"), line(""), line("")],
  [line("a"), line("\u3000\t", false)],
  [line("a\u00a0\ufeff")],
  [line("中文"), line("😀", true)],
  [line("e\u0301"), line("😀", true)],
  [line("x\n")],
  [line("a\u0085")],
].map((input) => ({ input: input.map(({ text, isWrapped }) => ({ text, isWrapped })), result: marks.joinBufferLines(input) }))

const redrawCases = [
  "\x1b[1G",
  "\x1b[0K",
  "\x1b[?25l\x1b[H\x1b[?25h",
  "\x1b[2K\x1b[1A\x1b[2K\x1b[G",
  "\x1b[12;1H\r",
  "\x1b[3F",
  "\r",
  "\x1b[?25l",
  "",
  "\x1b[?25l\x1b[30;12H\x1b[?25h",
  "\x1b[5G",
  "\x1b[1G\x1b[3C",
  "\r\b",
  "\x1b[2A",
  "\x1b[999999999999999999999999G",
  "\x1b[1;0H",
  "\x1b[1;1;9H",
  "\u3000",
  "\x1b[1G\n",
].map((text) => ({ text, result: cursorMoveHold.isLineRedrawLead(text) }))

const endsLineCases = ["", "abc", "a\r", "a\n", "a\u0003", "a\u0004", "😀\n"].map(
  (data) => ({ data, result: predictiveEcho.endsLine(data) })
)

class FakeLine {
  cursorRow = 5
  cols = 80
  constructor(text = "$ ", cursorCol = text.length) {
    this.text = text
    this.cursorCol = cursorCol
  }
  charAt(row, col) {
    return (row === this.cursorRow && this.text[col]) || " "
  }
  echo(typed) {
    for (const char of typed) {
      if (char === "\x7f") {
        this.cursorCol -= 1
        this.text = this.text.slice(0, this.cursorCol)
      } else {
        this.text =
          this.text.slice(0, this.cursorCol) + char + this.text.slice(this.cursorCol + 1)
        this.cursorCol += 1
      }
    }
  }
}

let predictorClock = 0
const predictor = new predictiveEcho.EchoPredictor(() => predictorClock)
const predictorLine = new FakeLine()
predictor.type("a", predictorLine)
predictorClock += 100
predictorLine.echo("a")
predictor.reconcile(predictorLine)
predictor.type("b你", predictorLine)
const afterUnicodeInput = {
  activeRow: predictor.activeRow,
  view: predictor.view(),
  busy: predictor.busy,
}

const predictiveUnicodeInputs = ["你", "😀", "e\u0301"].map((data) => {
  let unicodeClock = 0
  const unicodePredictor = new predictiveEcho.EchoPredictor(() => unicodeClock)
  const unicodeLine = new FakeLine()
  unicodePredictor.type("a", unicodeLine)
  unicodeClock += 100
  unicodeLine.echo("a")
  unicodePredictor.reconcile(unicodeLine)
  unicodePredictor.type(data, unicodeLine)
  return {
    data,
    activeRow: unicodePredictor.activeRow,
    view: unicodePredictor.view(),
    busy: unicodePredictor.busy,
  }
})

function fakeTerminal() {
  const handlers = new Map()
  let line = 0
  let column = 0
  let normal = true
  let bell = null
  let lineFeed = null
  const term = {
    buffer: { active: { type: "normal" } },
    parser: {
      registerOscHandler(code, listener) {
        const list = handlers.get(code) ?? []
        list.push(listener)
        handlers.set(code, list)
        return { dispose() {} }
      },
    },
    registerMarker() {
      return {
        line,
        isDisposed: false,
        dispose() {},
        onDispose() {},
      }
    },
    registerDecoration() {
      return { dispose() {}, onRender() {} }
    },
    onBell(listener) {
      bell = listener
      return { dispose() {} }
    },
    onLineFeed(listener) {
      lineFeed = listener
      return { dispose() {} }
    },
    get line() {
      return line
    },
    set line(value) {
      line = value
    },
    get column() {
      return column
    },
    get normal() {
      return normal
    },
    set normal(value) {
      normal = value
      term.buffer.active.type = value ? "normal" : "alternate"
    },
    send(code, data) {
      for (const listener of handlers.get(code) ?? []) listener(data)
    },
    fireBell() {
      bell?.()
    },
    fireLineFeed() {
      lineFeed?.()
    },
  }
  return term
}

const listenerStub = stubUrl(`
export function watchAttentionReports() { return { dispose() {} } }
export function watchCwdReports() { return { dispose() {} } }
export function watchOsReports() { return { dispose() {} } }
export function watchMouseReporting() { return { dispose() {} } }
`)
const agentStub = stubUrl(`
globalThis.__agentTrackerEvents = globalThis.__agentTrackerEvents ?? []
export class AgentTracker {
  constructor() { globalThis.__agentTracker = this; this.speaking = false }
  reset() { this.speaking = false; globalThis.__agentTrackerEvents.push("reset") }
  dispose() { globalThis.__agentTrackerEvents.push("dispose") }
  report() { this.speaking = true; return true }
  resumeSession() { return null }
  submit() {}
  get speaksForAgent() { return this.speaking }
}
`)
const remoteCwdStub = stubUrl(`
export function getTabRemoteHostname() { return null }
export function setTabRemoteAwayHost() {}
export function setTabRemoteCwd() {}
`)
const tabAttentionStub = stubUrl(`
export function setTabAgentStatus() {}
export function setTabRunningAgent() {}
export function setTabRunningCommand(_tab, command) {
  globalThis.__oracleRunningCommands = globalThis.__oracleRunningCommands ?? []
  globalThis.__oracleRunningCommands.push(command)
}
export function setTabPasswordPrompt() {}
`)
const invokeState = { calls: [], results: [] }
const invokeStub = stubUrl(`
export async function invoke(command, args) {
  globalThis.__oracleInvoke.calls.push({ command, args })
  const value = globalThis.__oracleInvoke.results.shift()
  return typeof value === "function" ? value() : value
}
`)
globalThis.__oracleInvoke = invokeState
const savedPasswordNoticeStub = stubUrl(`
globalThis.__oracleSavedNotices = globalThis.__oracleSavedNotices ?? []
export function notifySavedPasswordNotSent() { globalThis.__oracleSavedNotices.push("not-sent") }
`)

const trackerUrl = moduleUrl("src/components/TerminalTab/commandTracking.ts", {
  "@tauri-apps/api/core": invokeStub,
  "@/components/TerminalTab/commandMarks": commandMarksUrl,
  "@/components/TerminalTab/attentionReports": listenerStub,
  "@/components/TerminalTab/cwdReports": listenerStub,
  "@/components/TerminalTab/mouseReportingGuard": listenerStub,
  "@/components/TerminalTab/osReports": listenerStub,
  "@/lib/agentStatus": agentStub,
  "@/lib/remoteCwd": remoteCwdStub,
  "@/lib/tabAttention": tabAttentionStub,
  "@/lib/terminalNotifications": terminalNotificationsUrl,
  "@/lib/terminalCommandCapture": terminalCommandCaptureUrl,
})
const { createCommandTracking } = await import(trackerUrl)

let trackingNow = 1000
const originalNow = Date.now
Date.now = () => trackingNow

const trackingTerm = fakeTerminal()
const executed = []
const attention = []
const agentSessions = []
globalThis.__oracleRunningCommands = []
const ctx = {
  term: trackingTerm,
  session: { lineFeedCount: 0, lastPasswordFill: null },
  tabId: "tab",
  sessionNonce: 1,
  onAttentionRef: { current: (event) => attention.push(event) },
  connectionRef: { current: { type: "ssh", profileId: "p1", profileName: "web" } },
  onCommandExecutedRef: { current: (command) => executed.push(command) },
  commandMarksEnabledRef: { current: true },
  commandMarksRef: { current: null },
  onCwdChangeRef: { current: null },
  onDistroChangeRef: { current: null },
  onAgentSessionChangeRef: { current: (session) => agentSessions.push(session) },
}
const tracking = createCommandTracking(ctx)

const emptyTyped = tracking.emitTypedCommand("")
const whitespaceTyped = tracking.emitTypedCommand(" \u3000 ")
const firstTyped = tracking.emitTypedCommand("\u3000 ls -la \u3000")
const duplicateTyped = tracking.emitTypedCommand("ls -la")
trackingNow += 1499
const duplicateAt1499 = tracking.emitTypedCommand("ls -la")
trackingNow += 1
const duplicateAt1500 = tracking.emitTypedCommand("ls -la")
ctx.session.lastPasswordFill = { prompt: "old", at: trackingNow }
const afterNewCommandFill = tracking.emitTypedCommand("pwd")
const fillAfterNewCommand = ctx.session.lastPasswordFill
trackingNow += 1
trackingTerm.send(633, "E;echo reported")
const afterReported = tracking.emitTypedCommand("not restored")
trackingNow += 1000
trackingTerm.send(133, "A")
const agentEventsAfterPrompt = [...globalThis.__agentTrackerEvents]
const sessionsAfterPrompt = [...agentSessions]
trackingNow += 100
trackingTerm.send(633, "C")
trackingNow += 250
trackingTerm.send(633, "D;0")
const runningCommands = [...globalThis.__oracleRunningCommands]

const trackingOracle = {
  emptyTypedIsUndefined: emptyTyped === undefined,
  whitespaceTypedIsUndefined: whitespaceTyped === undefined,
  firstTyped,
  duplicateTyped,
  duplicateAt1499,
  duplicateAt1500,
  afterNewCommandFill,
  fillAfterNewCommand,
  afterReported,
  executed,
  attention,
  agentEventsAfterPrompt,
  sessionsAfterPrompt,
  runningCommands,
}

function freshTracking({
  marksEnabled = true,
  profileId = "p1",
  profileName = "web",
  omitProfile = false,
} = {}) {
  globalThis.__oracleRunningCommands = []
  globalThis.__agentTrackerEvents = []
  const term = fakeTerminal()
  const executed = []
  const attention = []
  const agentSessions = []
  const ctx = {
    term,
    session: { lineFeedCount: 0, lastPasswordFill: null },
    tabId: "tab",
    sessionNonce: 1,
    onAttentionRef: { current: (event) => attention.push(event) },
    connectionRef: {
      current: omitProfile ? { type: "ssh" } : { type: "ssh", profileId, profileName },
    },
    onCommandExecutedRef: { current: (command) => executed.push(command) },
    commandMarksEnabledRef: { current: marksEnabled },
    commandMarksRef: { current: null },
    onCwdChangeRef: { current: null },
    onDistroChangeRef: { current: null },
    onAgentSessionChangeRef: { current: (session) => agentSessions.push(session) },
  }
  return {
    tracking: createCommandTracking(ctx),
    term,
    ctx,
    executed,
    attention,
    agentSessions,
    running: () => [...globalThis.__oracleRunningCommands],
  }
}

trackingNow = 1000
const runningScenario = freshTracking()
runningScenario.tracking.emitTypedCommand("ls -la")
runningScenario.term.send(633, "C")
trackingNow += 100
runningScenario.term.send(633, "E;echo reported")
trackingNow += 250
runningScenario.term.send(633, "D;0")
const trackingRunningCommands = {
  executed: runningScenario.executed,
  attention: runningScenario.attention,
  runningCommands: runningScenario.running(),
}

trackingNow = 1000
const timerScenario = freshTracking()
timerScenario.tracking.emitTypedCommand("ls -la")
timerScenario.tracking.commandTimer.start(1000)
trackingNow = 1350
timerScenario.term.send(633, "D;0")
const commandTimerAccessor = {
  executed: timerScenario.executed,
  attention: timerScenario.attention,
  runningCommands: timerScenario.running(),
}

trackingNow = 1000
const edgeScenario = freshTracking({ omitProfile: true })
const edgeEmpty = edgeScenario.tracking.emitTypedCommand("")
const edgeWhitespace = edgeScenario.tracking.emitTypedCommand(" \u3000 ")
const edgeTyped = edgeScenario.tracking.emitTypedCommand(" uptime ")
edgeScenario.term.send(633, "B")
edgeScenario.term.send(633, "D;x")
const trackingEdgeCases = {
  emptyIsUndefined: edgeEmpty === undefined,
  whitespaceIsUndefined: edgeWhitespace === undefined,
  typed: edgeTyped,
  executed: edgeScenario.executed,
  attention: edgeScenario.attention,
  agentSessions: edgeScenario.agentSessions,
}

trackingNow = 1000
const agentResetScenarioSetup = freshTracking()
agentResetScenarioSetup.tracking.agentTracker.report({})
const agentSpeakingBeforePrompt = agentResetScenarioSetup.tracking.agentTracker.speaksForAgent
agentResetScenarioSetup.term.send(133, "A")
const agentResetScenario = {
  speaksBeforePrompt: agentSpeakingBeforePrompt,
  speaksAfterPrompt: agentResetScenarioSetup.tracking.agentTracker.speaksForAgent,
  events: [...globalThis.__agentTrackerEvents],
  sessions: [...agentResetScenarioSetup.agentSessions],
}
Date.now = originalNow

const savedUrl = moduleUrl("src/components/TerminalTab/savedPasswordPrompts.ts", {
  "@tauri-apps/api/core": invokeStub,
  "@/components/TerminalTab/savedPasswordNotice": savedPasswordNoticeStub,
  "@/lib/sudoPrompt": sudoPromptUrl,
  "@/lib/loginScript": loginScriptUrl,
  "@/lib/passwordFeedback": passwordFeedbackUrl,
  "@/lib/tabAttention": tabAttentionStub,
  "@/lib/terminalCommandCapture": terminalCommandCaptureUrl,
})
const { createSavedPasswordPrompts } = await import(savedUrl)

const SUDO_PROMPT = "[sudo] password for alice:"
const NO_USER_PROMPT = "[sudo: authenticate] Password:"
let savedNow = 100_000
Date.now = () => savedNow

function savedSetup({
  username = "alice",
  omitUsername = false,
  profileId = "p1",
  profileName = "web",
  source = null,
  onInput = undefined,
  line = SUDO_PROMPT,
  patterns = [],
} = {}) {
  const shown = []
  const offers = []
  const sensitive = []
  let inputRequests = []
  const buffer = {
    baseY: 0,
    cursorY: 0,
    cursorX: line.length,
    getLine: () => ({
      isWrapped: false,
      translateToString: (_trim, start = 0, end = currentLine.length) =>
        currentLine.slice(start, end),
    }),
  }
  let currentLine = line
  const ctx = {
    term: { buffer: { active: buffer } },
    session: {
      disposed: false,
      passwordPromptCheckId: 0,
      commandCaptureState: {},
      commandCaptureSuspended: false,
      lineFeedCount: 0,
      currentPromptKey: null,
      lastPasswordFill: null,
      loginScriptRunner: null,
    },
    tabId: "tab",
    sessionNonce: 1,
    connectionRef: {
      current: {
        type: "ssh",
        ...(omitUsername ? {} : { username }),
        profileId,
        profileName,
        loginScript: "cd /srv\r\n\r\necho ready\r\n",
      },
    },
    onSensitivePromptRef: { current: () => sensitive.push("sensitive") },
    onSavedPasswordPromptChange: (...args) => offers.push(args),
    setSavedPasswordPrompt: (state) => shown.push(state),
    sudoPromptPatternsRef: { current: patterns },
    onInputRef: {
      current: onInput
        ? async (request) => {
            inputRequests.push(request)
            return true
          }
        : undefined,
    },
    translationRef: { current: (key) => key },
    savedPasswordPromptActionsRef: { current: null },
  }
  const prompts = createSavedPasswordPrompts(ctx)
  const check = async (nextLine) => {
    currentLine = nextLine
    buffer.cursorX = nextLine.length
    prompts.checkPasswordPrompt()
    await Promise.resolve()
    await Promise.resolve()
    await Promise.resolve()
  }
  return {
    ctx,
    prompts,
    shown,
    offers,
    sensitive,
    check,
    get inputRequests() {
      return inputRequests
    },
  }
}

async function lookupScenario(options, line = SUDO_PROMPT) {
  invokeState.calls = []
  invokeState.results = [options.source]
  const setup = savedSetup(options)
  await setup.check(line)
  return {
    line,
    source: options.source,
    shown: setup.shown,
    offers: setup.offers,
    sensitive: setup.sensitive.length,
    calls: invokeState.calls,
    currentPromptKey: setup.ctx.session.currentPromptKey,
    passwordPromptCheckId: setup.ctx.session.passwordPromptCheckId,
    commandCaptureSuspended: setup.ctx.session.commandCaptureSuspended,
    commandCaptureState: setup.ctx.session.commandCaptureState,
  }
}

const lookupScenarios = {
  savedSource: await lookupScenario({ source: "login" }),
  emptySource: await lookupScenario({ source: "" }),
  nullSource: await lookupScenario({ source: null }),
  otherUser: await lookupScenario({ source: "login" }, "[sudo] password for root:"),
  emptyProfile: await lookupScenario({ profileId: "", profileName: "", source: "login" }),
  nameOnlyProfile: await lookupScenario({ profileId: "", profileName: "web", source: "sudo" }),
  noUserPrompt: await lookupScenario({ source: "login" }, NO_USER_PROMPT),
  emptyUsernameNoUserPrompt: await lookupScenario(
    { username: "", source: "login" },
    NO_USER_PROMPT
  ),
}

async function fillScenario({ onInput, written = true } = {}) {
  savedNow = 50_000
  invokeState.calls = []
  globalThis.__oracleSavedNotices = []
  invokeState.results = ["login", written]
  const setup = savedSetup({ source: "login", onInput })
  await setup.check(SUDO_PROMPT)
  const result = setup.ctx.savedPasswordPromptActionsRef.current.fill()
  const second = setup.ctx.savedPasswordPromptActionsRef.current.fill()
  await Promise.resolve()
  await Promise.resolve()
  return {
    result,
    second,
    calls: invokeState.calls,
    inputRequests: setup.inputRequests,
    lastPasswordFill: setup.ctx.session.lastPasswordFill,
    shown: setup.shown,
    commandCaptureSuspended: setup.ctx.session.commandCaptureSuspended,
    commandCaptureState: setup.ctx.session.commandCaptureState,
    notices: [...globalThis.__oracleSavedNotices],
  }
}

const fill = await fillScenario()
const fillThroughInput = await fillScenario({ onInput: true })
const fillRejected = await fillScenario({ written: false })

async function lastFillScenario(offset) {
  invokeState.calls = []
  invokeState.results = ["login"]
  const setup = savedSetup({ source: "login" })
  setup.ctx.session.lastPasswordFill = {
    prompt: SUDO_PROMPT,
    at: 100_000 - offset,
  }
  await setup.check(SUDO_PROMPT)
  return {
    offset,
    shown: setup.shown,
    calls: invokeState.calls,
    sensitive: setup.sensitive.length,
    passwordPromptCheckId: setup.ctx.session.passwordPromptCheckId,
    commandCaptureSuspended: setup.ctx.session.commandCaptureSuspended,
    commandCaptureState: setup.ctx.session.commandCaptureState,
  }
}

savedNow = 100_000
const rejectionScenarios = {
  oneMillisecondBeforeWindow: await lastFillScenario(19_999),
  atWindow: await lastFillScenario(20_000),
  differentPrompt: await (async () => {
    invokeState.calls = []
    invokeState.results = ["login"]
    const setup = savedSetup({ source: "login" })
    setup.ctx.session.lastPasswordFill = { prompt: "other", at: 100_000 }
    await setup.check(SUDO_PROMPT)
    return {
      shown: setup.shown,
      calls: invokeState.calls,
      sensitive: setup.sensitive.length,
    }
  })(),
}

async function staleLookupScenario() {
  let resolveLookup
  invokeState.calls = []
  invokeState.results = [
    () =>
      new Promise((resolve) => {
        resolveLookup = resolve
      }),
  ]
  const setup = savedSetup({ source: "login" })
  setup.prompts.checkPasswordPrompt()
  await Promise.resolve()
  await setup.check("$ ")
  resolveLookup("login")
  await Promise.resolve()
  await Promise.resolve()
  await Promise.resolve()
  return {
    shown: setup.shown,
    offers: setup.offers,
    calls: invokeState.calls,
  }
}

const staleLookup = await staleLookupScenario()

async function failedLookupScenario() {
  invokeState.calls = []
  invokeState.results = [
    () => {
      throw new Error("lookup failed")
    },
  ]
  const setup = savedSetup()
  const originalConsoleError = console.error
  console.error = () => {}
  try {
    await setup.check(SUDO_PROMPT)
  } finally {
    console.error = originalConsoleError
  }
  return {
    shown: setup.shown,
    offers: setup.offers,
    sensitive: setup.sensitive.length,
    calls: invokeState.calls,
  }
}

const failedLookup = await failedLookupScenario()
Date.now = originalNow

function loginScriptScenario(connection, script) {
  const setup = savedSetup()
  setup.ctx.connectionRef.current = {
    type: connection,
    username: "alice",
    profileId: "p1",
    profileName: "web",
    loginScript: script,
  }
  setup.prompts.startLoginScript()
  return {
    connection,
    script,
    lines: loginScriptModule.parseLoginScript(script),
    hasRunner: setup.ctx.session.loginScriptRunner !== null,
  }
}

async function lineFeedScenario() {
  invokeState.calls = []
  invokeState.results = ["login"]
  const setup = savedSetup({ source: "login" })
  setup.ctx.session.lineFeedCount = 7
  await setup.check(SUDO_PROMPT)
  return {
    currentPromptKey: setup.ctx.session.currentPromptKey,
    shown: setup.shown,
  }
}

async function typingUnicodeScenario() {
  invokeState.calls = []
  invokeState.results = [null]
  const setup = savedSetup()
  await setup.check(SUDO_PROMPT)
  setup.prompts.typeAtPrompt("密😀e\u0301")
  const afterUnicode = setup.shown.at(-1)
  setup.prompts.typeAtPrompt("")
  const afterEmpty = setup.shown.at(-1)
  setup.prompts.typeAtPrompt("\x7f")
  const afterBackspace = setup.shown.at(-1)
  return { afterUnicode, afterEmpty, afterBackspace }
}

async function customPromptUserScenario({ username, omitUsername, line }) {
  invokeState.calls = []
  invokeState.results = ["login"]
  const setup = savedSetup({
    username,
    omitUsername,
    source: "login",
    patterns: [new RegExp("^PIN:(?<user>.*)$")],
  })
  await setup.check(line)
  return {
    username,
    omitUsername,
    line,
    shown: setup.shown,
    offers: setup.offers,
    sensitive: setup.sensitive.length,
    calls: invokeState.calls,
    currentPromptKey: setup.ctx.session.currentPromptKey,
  }
}

async function namedPromptWithoutUsernameScenario() {
  invokeState.calls = []
  invokeState.results = ["login"]
  const setup = savedSetup({ omitUsername: true, source: "login" })
  await setup.check(SUDO_PROMPT)
  return {
    shown: setup.shown,
    offers: setup.offers,
    sensitive: setup.sensitive.length,
    calls: invokeState.calls,
    currentPromptKey: setup.ctx.session.currentPromptKey,
  }
}

async function rejectedLockoutScenario() {
  savedNow = 100_000
  Date.now = () => savedNow
  invokeState.calls = []
  invokeState.results = [null]
  const setup = savedSetup()
  setup.ctx.session.lastPasswordFill = { prompt: SUDO_PROMPT, at: 80_001 }
  await setup.check(SUDO_PROMPT)
  const rejected = {
    shown: [...setup.shown],
    calls: invokeState.calls.length,
  }
  setup.ctx.session.lineFeedCount += 1
  invokeState.calls = []
  await setup.check(SUDO_PROMPT)
  return {
    rejected,
    afterNewPrompt: {
      shown: setup.shown,
      offers: setup.offers,
      sensitive: setup.sensitive.length,
      calls: invokeState.calls,
      currentPromptKey: setup.ctx.session.currentPromptKey,
    },
  }
}

const savedOracle = {
  lookupScenarios,
  fill,
  fillThroughInput,
  fillRejected,
  rejectionScenarios,
  staleLookup,
  failedLookup,
  loginScriptCases: [
    loginScriptScenario("ssh", "cd /srv\r\n\r\necho ready\r\n"),
    loginScriptScenario("terminal", null),
    loginScriptScenario("other", "echo skipped"),
    loginScriptScenario("ssh", ""),
    loginScriptScenario("ssh", " \u3000\r\n"),
  ],
  lineFeedKey: await lineFeedScenario(),
  typingUnicode: await typingUnicodeScenario(),
  customPromptUsers: {
    missingUsername: await customPromptUserScenario({
      omitUsername: true,
      line: "PIN:",
    }),
    emptyUsername: await customPromptUserScenario({
      username: "",
      line: "PIN:",
    }),
  },
  namedPromptWithoutUsername: await namedPromptWithoutUsernameScenario(),
  rejectedLockout: await rejectedLockoutScenario(),
}

console.log(
  JSON.stringify(
    {
      markCases,
      markPromptEventCases,
      joinCases,
      redrawCases,
      endsLineCases,
      afterUnicodeInput,
      predictiveUnicodeInputs,
      trackingOracle,
      trackingRunningCommands,
      commandTimerAccessor,
      trackingEdgeCases,
      agentResetScenario,
      savedOracle,
    },
    null,
    2
  )
)
