// Runs the real TypeScript workspace modules without a browser or changes to
// src/. The React stub evaluates hooks as plain getters so the hook-shaped
// exports can be observed; the DOM stub is jsdom for tabNudge.
import fs from "node:fs"
import ts from "typescript"
import { JSDOM } from "jsdom"

function dataUrl(source) {
  return `data:text/javascript;base64,${Buffer.from(source).toString("base64")}`
}

function moduleUrl(path, imports = {}) {
  let source = ts.transpileModule(fs.readFileSync(path, "utf8"), {
    compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ESNext },
  }).outputText
  for (const [specifier, url] of Object.entries(imports)) {
    source = source
      .replaceAll(`"${specifier}"`, JSON.stringify(url))
      .replaceAll(`'${specifier}'`, JSON.stringify(url))
  }
  return dataUrl(source)
}

const reactUrl = dataUrl(`
export function useSyncExternalStore(_subscribe, getSnapshot) { return getSnapshot() }
`)

const tabTypesUrl = moduleUrl("src/types/tab.ts")
const brandLogosUrl = moduleUrl("src/lib/brandLogos.ts")
const distroUrl = moduleUrl("src/lib/distro.ts")
const tabGroupsUrl = moduleUrl("src/lib/tabGroups.ts")
const tabIconsUrl = moduleUrl("src/lib/tabIcons.ts", {
  "@/lib/brandLogos": brandLogosUrl,
  "@/lib/distro": distroUrl,
  "@/types/tab": tabTypesUrl,
})
const terminalNotificationsUrl = moduleUrl("src/lib/terminalNotifications.ts")
const terminalLogRecordingUrl = moduleUrl("src/lib/terminalLogRecording.ts", {
  react: reactUrl,
  "@/types/tab": tabTypesUrl,
})
const tabGroupsPersistenceUrl = moduleUrl("src/lib/tabGroupsPersistence.ts", {
  "@/lib/tabGroups": tabGroupsUrl,
})
const paneLayoutUrl = moduleUrl("src/lib/paneLayout.ts", {
  "@/lib/tabGroups": tabGroupsUrl,
})
const paneNavigationUrl = moduleUrl("src/lib/paneNavigation.ts")
const paneTemplatesUrl = moduleUrl("src/lib/paneTemplates.ts", {
  "@/lib/tabGroups": tabGroupsUrl,
})
const tabNavigationUrl = moduleUrl("src/lib/tabNavigation.ts")
const tabClosingUrl = moduleUrl("src/lib/tabClosing.ts")
const tabMenuActionsUrl = moduleUrl("src/lib/tabMenuActions.ts", {
  "@/lib/tabIcons": tabIconsUrl,
  "@/lib/terminalLogRecording": terminalLogRecordingUrl,
  "@/types/tab": tabTypesUrl,
})
const tabAttentionUrl = moduleUrl("src/lib/tabAttention.ts", {
  react: reactUrl,
  "@/lib/terminalNotifications": terminalNotificationsUrl,
})
const tabAgentIndicatorUrl = moduleUrl("src/lib/tabAgentIndicator.ts")
const appNavigationUrl = moduleUrl("src/lib/appNavigation.ts")
const tabNudgeUrl = moduleUrl("src/lib/tabNudge.ts")

const tabGroups = await import(tabGroupsUrl)
const persistence = await import(tabGroupsPersistenceUrl)
const paneLayout = await import(paneLayoutUrl)
const paneNavigation = await import(paneNavigationUrl)
const paneTemplates = await import(paneTemplatesUrl)
const tabNavigation = await import(tabNavigationUrl)
const tabClosing = await import(tabClosingUrl)
const tabMenuActions = await import(tabMenuActionsUrl)
const tabAttention = await import(tabAttentionUrl)
const terminalNotifications = await import(terminalNotificationsUrl)
const terminalLogRecording = await import(terminalLogRecordingUrl)
const tabAgentIndicator = await import(tabAgentIndicatorUrl)
const appNavigation = await import(appNavigationUrl)
const tabNudge = await import(tabNudgeUrl)

const tree = {
  type: "split",
  orientation: "horizontal",
  sizes: [0.5, 0.5],
  children: [
    tabGroups.paneLeaf("a"),
    {
      type: "split",
      orientation: "vertical",
      sizes: [0.25, 0.75],
      children: [tabGroups.paneLeaf("b"), tabGroups.paneLeaf("c")],
    },
  ],
}
const area = { left: 0, top: 0, width: 1005, height: 405 }
const layout = paneLayout.layoutPaneTree(tree, area, 5)
const negativeLayout = paneLayout.layoutPaneTree(
  {
    type: "split",
    orientation: "horizontal",
    sizes: [0.5, 0.5],
    children: [tabGroups.paneLeaf("a"), tabGroups.paneLeaf("b")],
  },
  { left: -10, top: -3, width: 5, height: 10 },
  0
)
const dragCases = [
  { sizes: [0.5, 0.5], index: 0, delta: -37, room: 1003, minimums: [200, 200] },
  { sizes: [0.5, 0.5], index: 0, delta: 37, room: 1003, minimums: [200, 200] },
  {
    sizes: [0.3333333333333333, 0.6666666666666666],
    index: 0,
    delta: -17,
    room: 997,
    minimums: [100, 200],
  },
  { sizes: [0.25, 0.75], index: 0, delta: 11, room: 999, minimums: [100, 300] },
]
const jsNumberText = (value) => (Object.is(value, -0) ? "-0" : String(value))
const roundInputs = [
  0.49999999999999994,
  -0.49999999999999994,
  0.5,
  -0.5,
  1.5,
  -1.5,
  2.5,
  -2.5,
  -0.4,
  -0.6,
  0,
  -0,
  4503599627370495.5,
  4503599627370496,
  4503599627370497,
  -4503599627370495.5,
  -4503599627370496,
  -4503599627370497,
  1e300,
  -1e300,
  Number.NaN,
  Number.POSITIVE_INFINITY,
  Number.NEGATIVE_INFINITY,
]
const rect = (id, left, top, right, bottom) => ({ id, left, top, right, bottom })
const capture = (call) => {
  try {
    return { value: call() ?? null }
  } catch (error) {
    return { error: error.constructor.name }
  }
}
const tallLeft = [
  rect("c", 508, 304, 1000, 600),
  rect("a", 0, 0, 500, 600),
  rect("b", 508, 0, 1000, 296),
]
const readingOrderStress = [
  rect("L1", 0, 0, 496, 200),
  rect("L2", 0, 209, 496, 600),
  rect("M1", 504, 0, 1000, 200),
  rect("M2", 504, 207, 1000, 600),
  rect("R2", 1008, 205, 1504, 600),
  rect("R1", 1008, 0, 1504, 200),
]
const emptyIdPanes = [rect("", 0, 0, 50, 50), rect("x", 58, 0, 100, 50)]
const readingOrderSeventy = Array.from({ length: 70 }, (_, index) => {
  const group = index % 3
  const left = (2 - group) * 1000 + index
  const top = group * 2
  return rect(`p${index}`, left, top, left + 100, top + 100)
})

function permutations(items) {
  const result = []
  const visit = (start) => {
    if (start === items.length) {
      result.push(items.slice())
      return
    }
    for (let index = start; index < items.length; index += 1) {
      ;[items[start], items[index]] = [items[index], items[start]]
      visit(start + 1)
      ;[items[start], items[index]] = [items[index], items[start]]
    }
  }
  visit(0)
  return result
}

const sortedReadingOrderIds = (panes) =>
  paneNavigation.sortPanesByReadingOrder(panes).map((pane) => pane.id)

const nestedShape = {
  type: "split",
  orientation: "horizontal",
  sizes: [2, 2],
  children: [
    { type: "pane" },
    {
      type: "split",
      orientation: "vertical",
      children: [{ type: "pane" }, { type: "pane" }],
    },
  ],
}
const tooDeep = {
  type: "split",
  orientation: "horizontal",
  children: [
    {
      type: "split",
      orientation: "vertical",
      children: [
        {
          type: "split",
          orientation: "horizontal",
          children: [
            {
              type: "split",
              orientation: "vertical",
              children: [
                {
                  type: "split",
                  orientation: "horizontal",
                  children: [{ type: "pane" }, { type: "pane" }],
                },
                { type: "pane" },
              ],
            },
            { type: "pane" },
          ],
        },
        { type: "pane" },
      ],
    },
    { type: "pane" },
  ],
}
const deepestPane = {
  type: "split",
  orientation: "horizontal",
  children: [
    {
      type: "split",
      orientation: "vertical",
      children: [
        {
          type: "split",
          orientation: "horizontal",
          children: [
            {
              type: "split",
              orientation: "vertical",
              children: [{ type: "pane" }, { type: "pane" }],
            },
            { type: "pane" },
          ],
        },
        { type: "pane" },
      ],
    },
    { type: "pane" },
  ],
}
const groups = [
  {
    id: "g1",
    root: tabGroups.normalizePaneTree(tree),
    activeTabId: "c",
    zoomedTabId: "b",
  },
  { id: "g2", root: tabGroups.paneLeaf("d"), activeTabId: "d" },
]
const saved = persistence.serializeTabGroups(groups, new Set(["a", "b", "c", "d"]))
const t = (key) => key
const terminal = { id: "a", title: "tTerm", type: "terminal", isActive: true }

const dom = new JSDOM(`
  <div class="tab-list-viewport">
    <div class="tab-item" data-tab-id="t1"><span class="tab-agent"></span></div>
    <div class="tab-item" data-tab-id="t2"></div>
  </div>`)
globalThis.document = dom.window.document
globalThis.Element = dom.window.Element
globalThis.Event = dom.window.Event
let scrollCalls = 0
dom.window.Element.prototype.scrollIntoView = () => {
  scrollCalls += 1
}
const tab = (id) => document.querySelector(`[data-tab-id="${id}"]`)
const animationEnd = (target) => {
  target.dispatchEvent(new dom.window.Event("animationend", { bubbles: true }))
}
tabNudge.nudgeTab("t2")
const nudgeNamed = {
  t2Flashing: tab("t2").classList.contains("tab-nudge-flash"),
  t1Flashing: tab("t1").classList.contains("tab-nudge-flash"),
  scrollCalls,
}
animationEnd(tab("t2"))
scrollCalls = 0
tabNudge.nudgeTab("t1")
animationEnd(tab("t1").querySelector(".tab-agent"))
const nudgeChildEnd = tab("t1").classList.contains("tab-nudge-flash")
animationEnd(tab("t1"))
const nudgeOwnEnd = tab("t1").classList.contains("tab-nudge-flash")
tabNudge.nudgeTab("t1")
tabNudge.nudgeTab("t1")
const nudgeReplayRunning = tab("t1").classList.contains("tab-nudge-flash")
animationEnd(tab("t1"))
const nudgeReplayEnded = tab("t1").classList.contains("tab-nudge-flash")
tabNudge.nudgeTab("t1")
const nudgeReplayAgain = tab("t1").classList.contains("tab-nudge-flash")
animationEnd(tab("t1"))
tabNudge.nudgeTab("missing")
const nudgeMissing = document.querySelector(".tab-nudge-flash") === null
const bar = document.querySelector(".tab-list-viewport")
tabNudge.nudgeTabBar()
const nudgeBarRunning = bar.classList.contains("tab-nudge-shake")
animationEnd(bar)
const nudgeBarEnded = bar.classList.contains("tab-nudge-shake")
bar.remove()
tabNudge.nudgeTabBar()
const nudgeBarAbsent = document.querySelector(".tab-nudge-shake") === null

const totals = []
const stopTotals = tabAttention.watchUnseenNotificationTotal((total) => totals.push(total))
tabAttention.countTabNotification("oracle-1")
tabAttention.countTabNotification("oracle-1")
tabAttention.countTabNotification("oracle-2")
tabAttention.clearTabAttention("oracle-1")
tabAttention.clearTabAttention("oracle-2")
stopTotals()
let markListeners = 0
const stopMarks = tabAttention.watchUnseenNotificationTotal(() => {
  markListeners += 1
})
tabAttention.markTabAttention("oracle-3", "bell")
tabAttention.clearTabAttention("oracle-3")
stopMarks()
let stoppedListeners = 0
const stopStopped = tabAttention.watchUnseenNotificationTotal(() => {
  stoppedListeners += 1
})
stopStopped()
tabAttention.countTabNotification("oracle-4")
tabAttention.clearTabAttention("oracle-4")

const realSetTimeout = globalThis.setTimeout
const realClearTimeout = globalThis.clearTimeout
let nextTimerId = 0
let scheduledTimers = []
globalThis.setTimeout = (callback, delay) => {
  nextTimerId += 1
  scheduledTimers.push({ id: nextTimerId, callback, delay })
  return nextTimerId
}
globalThis.clearTimeout = (id) => {
  scheduledTimers = scheduledTimers.filter((timer) => timer.id !== id)
}
const runScheduledTimers = () => {
  const timers = scheduledTimers
  scheduledTimers = []
  for (const timer of timers) timer.callback()
}

tabAttention.setTabRunningCommand("oracle-5", "npm run dev")
const commandScheduledDelay = scheduledTimers[0]?.delay ?? null
const commandImmediately = tabAttention.useRunningCommands().get("oracle-5") ?? null
runScheduledTimers()
const commandAfterDelay = tabAttention.useRunningCommands().get("oracle-5") ?? null
tabAttention.setTabRunningCommand("oracle-5", null)
const commandAfterClear = tabAttention.useRunningCommands().get("oracle-5") ?? null
tabAttention.setTabRunningCommand("oracle-6", "cd src")
tabAttention.setTabRunningCommand("oracle-6", "cd /tmp")
const replacementPendingTimers = scheduledTimers.length
const replacementBeforeDelay = tabAttention.useRunningCommands().get("oracle-6") ?? null
runScheduledTimers()
const replacementAfterDelay = tabAttention.useRunningCommands().get("oracle-6") ?? null
tabAttention.setTabRunningCommand("oracle-6", null)
const replacementAfterClear = tabAttention.useRunningCommands().get("oracle-6") ?? null
tabAttention.setTabRunningCommand("oracle-7", "cd src")
tabAttention.setTabRunningCommand("oracle-7", null)
const immediateTimersAfterClear = scheduledTimers.length
runScheduledTimers()
const commandEndedImmediately = tabAttention.useRunningCommands().get("oracle-7") ?? null

let clearedTotal = -1
const stopClearWatch = tabAttention.watchUnseenNotificationTotal((total) => {
  clearedTotal = total
})
tabAttention.markTabAttention("oracle-clear", "notification")
tabAttention.countTabNotification("oracle-clear")
const clearBefore = {
  mark: tabAttention.getTabAttention("oracle-clear") ?? null,
  total: clearedTotal,
}
tabAttention.clearTabAttention("oracle-clear")
const clearAfter = {
  mark: tabAttention.getTabAttention("oracle-clear") ?? null,
  total: clearedTotal,
}
stopClearWatch()
globalThis.setTimeout = realSetTimeout
globalThis.clearTimeout = realClearTimeout

tabAttention.setTabPasswordPrompt("oracle-pane-2", true)
const passwordWaiting = tabAttention.useTabPasswordPrompt(["oracle-pane-1", "oracle-pane-2"])
tabAttention.setTabPasswordPrompt("oracle-pane-2", false)
const passwordCleared = tabAttention.useTabPasswordPrompt(["oracle-pane-1", "oracle-pane-2"])
tabAttention.setTabAgentStatus("oracle-pane-1", { agent: "claude", activity: "waiting" })
tabAttention.setTabRunningAgent("oracle-pane-2", "codex")
tabAttention.markTabAttention("oracle-pane-1", "bell")
tabAttention.markTabAttention("oracle-pane-2", "activity")
const agentQuery = {
  attention: tabAttention.useTabAttention(["oracle-pane-1", "oracle-pane-2"]),
  status: tabAttention.useTabAgentStatus(["oracle-pane-1", "oracle-pane-2"]),
  runningAgent: tabAttention.useTabRunningAgent(["oracle-pane-1", "oracle-pane-2"]),
  missingStatus: tabAttention.useTabAgentStatus(["missing"]) ?? null,
  missingAgent: tabAttention.useTabRunningAgent(["missing"]) ?? null,
  noAttention: tabAttention.useTabAttention(["missing"]) ?? null,
  stronger: [
    terminalNotifications.strongerAttention("activity", "bell"),
    terminalNotifications.strongerAttention("notification", "bell"),
    terminalNotifications.strongerAttention(undefined, "activity"),
  ],
}
tabAttention.setTabRunningAgent("oracle-pane-2", null)
tabAttention.setTabAgentStatus("oracle-pane-1", null)

const logMenuIdle = terminalLogRecording.getTabLogMenuActions(terminal, t)
terminalLogRecording.setRecordingTabIds(["a"])
const logMenuRecording = terminalLogRecording.getTabLogMenuActions(terminal, t)
const logMenuRecordingSettings = terminalLogRecording.getTabLogMenuActions(
  { id: "settings", title: "Settings", type: "settings", isActive: true },
  t
)
const paneTerminalRecording = tabMenuActions.getPaneMenuActions(
  terminal,
  { isZoomed: false },
  t
)
terminalLogRecording.setRecordingTabIds([])

const opened = []
const notified = []
const stopHandling = appNavigation.handleAppPageRequests((page) => opened.push(page))
const stopListening = appNavigation.onLogsTabRequest(() => notified.push(1))
appNavigation.showLogsForTab({ tabId: "tab-1", sessionType: "local" })
const firstRequest = appNavigation.takeLogsTabRequest()
const secondRequest = appNavigation.takeLogsTabRequest()
stopListening()
stopHandling()
const firstHandler = []
const stopFirst = appNavigation.handleAppPageRequests((page) => firstHandler.push(page))
const secondHandler = []
const stopSecond = appNavigation.handleAppPageRequests((page) => secondHandler.push(page))
stopFirst()
appNavigation.showLogsForTab({ tabId: "tab-2", sessionType: "local" })
const replacedRequest = appNavigation.takeLogsTabRequest()
stopSecond()
appNavigation.showLogsForTab({
  tabId: "tab-3",
  sessionType: "ssh",
  host: "example.test",
  port: 22,
  username: "stone",
})
const noHandlerRequest = appNavigation.takeLogsTabRequest()
const noHandlerOpened = opened.length

const result = {
  paneLayout: {
    panes: [...layout.panes.entries()],
    sashes: layout.sashes,
    minimum: [
      paneLayout.minimumExtent(tree, "horizontal", { width: 200, height: 100 }, 5),
      paneLayout.minimumExtent(tree, "vertical", { width: 200, height: 100 }, 5),
    ],
    drag: {
      moved: paneLayout.dragSash([0.5, 0.5], 0, 100, 1000, [200, 200]),
      clamped: paneLayout.dragSash([0.5, 0.5], 0, 400, 1000, [200, 200]),
      invalidRoom: paneLayout.dragSash([0.5, 0.5], 0, 100, 0, [200, 200]),
      invalidIndex: paneLayout.dragSash([0.5, 0.5], 1, 100, 1000, [200, 200]),
      cases: dragCases.map((candidate) => {
        const result = paneLayout.dragSash(
          candidate.sizes,
          candidate.index,
          candidate.delta,
          candidate.room,
          candidate.minimums
        )
        return { ...candidate, result, resultJson: JSON.stringify(result) }
      }),
    },
    round: roundInputs.map((input) => ({
      input: jsNumberText(input),
      output: jsNumberText(Math.round(input)),
    })),
    split: [
      paneLayout.getSplitAt(tree, [1])?.orientation ?? null,
      paneLayout.getSplitAt(tree, [0]) ?? null,
    ],
    negativeLayout: [...negativeLayout.panes.entries()],
    numberSizesJson: JSON.stringify({
      type: "split",
      orientation: "horizontal",
      children: [
        { type: "pane", tabId: "a" },
        { type: "pane", tabId: "b" },
        { type: "pane", tabId: "c" },
        { type: "pane", tabId: "d" },
        { type: "pane", tabId: "e" },
      ],
      sizes: [1, 0.5, 0.000001, 0.0000001, 1 / 3],
    }),
    numberBoundsJson: JSON.stringify({
      type: "split",
      orientation: "horizontal",
      children: ["a", "b", "c", "d", "e", "f", "g"].map(tabGroups.paneLeaf),
      sizes: [-0, 1e21, 0.0000001, -0.0000001, NaN, Infinity, 1 / 3],
    }),
  },
  paneNavigation: {
    readingOrder: {
      example: {
        input: readingOrderStress.map((pane) => pane.id),
        sorted: sortedReadingOrderIds(readingOrderStress),
        sibling: paneNavigation.getSiblingPaneId(readingOrderStress, "R1", 1) ?? null,
      },
      permutations: permutations(readingOrderStress).map((input) => ({
        input: input.map((pane) => pane.id),
        sorted: sortedReadingOrderIds(input),
        sibling: paneNavigation.getSiblingPaneId(input, "R1", 1) ?? null,
      })),
      seventyNative: sortedReadingOrderIds(readingOrderSeventy),
    },
    emptyId: {
      sibling: paneNavigation.getSiblingPaneId(emptyIdPanes, "", 1) ?? null,
      direction: paneNavigation.findPaneInDirection(emptyIdPanes, "", "right") ?? null,
    },
    sorted: paneNavigation.sortPanesByReadingOrder([
      rect("right", 504, 0, 1000, 600),
      rect("left", 0, 1, 496, 600),
    ]),
    tolerance: {
      exactlyTwo: paneNavigation.sortPanesByReadingOrder([
        rect("right", 504, 0, 1000, 600),
        rect("left", 0, 2, 496, 600),
      ]),
      moreThanTwo: paneNavigation.sortPanesByReadingOrder([
        rect("right", 504, 0, 1000, 600),
        rect("left", 0, 3, 496, 600),
      ]),
    },
    sibling: {
      negativeWrap: capture(() => paneNavigation.getSiblingPaneId(tallLeft, "a", -4)),
      emptyActive: paneNavigation.getSiblingPaneId(tallLeft, "", 1) ?? null,
    },
    direction: {
      exactlyNegativeTwo: paneNavigation.findPaneInDirection(
        [rect("a", 0, 0, 50, 50), rect("b", 48, 0, 100, 50)],
        "a",
        "right"
      ),
      beyondNegativeTwo: paneNavigation.findPaneInDirection(
        [rect("a", 0, 0, 50, 50), rect("b", 47, 0, 100, 50)],
        "a",
        "right"
      ) ?? null,
      tieRecent: paneNavigation.findPaneInDirection(tallLeft, "a", "right", ["c", "a", "b"]),
      tieNoRecent: paneNavigation.findPaneInDirection(tallLeft, "a", "right", []),
      emptyActive: paneNavigation.findPaneInDirection(tallLeft, "", "right") ?? null,
    },
  },
  paneTemplates: {
    builtins: paneTemplates.BUILTIN_PANE_TEMPLATES.map((template) => ({
      id: template.id,
      labelKey: template.labelKey,
      name: template.name,
      root: template.root,
    })),
    isBuiltin: [
      paneTemplates.isBuiltinTemplateId("builtin:columns-2"),
      paneTemplates.isBuiltinTemplateId("custom"),
      paneTemplates.isBuiltinTemplateId("builtin:"),
    ],
    leafCount: paneTemplates.countTemplatePanes({ type: "pane" }),
    emptyFill: JSON.stringify(paneTemplates.fillShape({ type: "pane" }, [])),
    counts: paneTemplates.BUILTIN_PANE_TEMPLATES.map((template) =>
      paneTemplates.countTemplatePanes(template.root)
    ),
    shapeOfTree: paneTemplates.shapeOfTree(tree),
    filled: paneTemplates.fillShape(paneTemplates.shapeOfTree(tree), ["x", "y", "z"]),
    normalized: {
      nested: paneTemplates.normalizeLayoutShape(nestedShape),
      tooDeep: paneTemplates.normalizeLayoutShape(tooDeep),
      deepestPane: paneTemplates.normalizeLayoutShape(deepestPane),
      missingSizes: paneTemplates.normalizeLayoutShape({
        type: "split",
        orientation: "horizontal",
        children: [{ type: "pane" }, { type: "pane" }],
      }),
      zeroSizes: paneTemplates.normalizeLayoutShape({
        type: "split",
        orientation: "horizontal",
        sizes: [0, 0],
        children: [{ type: "pane" }, { type: "pane" }],
      }),
      lonePane: paneTemplates.normalizeLayoutShape({ type: "pane" }),
    },
    templates: paneTemplates.normalizePaneTemplates([
      {
        id: "t1",
        name: "\u3000Ops\uFEFF",
        root: nestedShape,
      },
      { id: "builtin:columns-2", root: nestedShape },
      { id: "t1", name: "again", root: nestedShape },
      { id: "", name: "empty", root: nestedShape },
      "nonsense",
    ]),
    emptyIdTemplate: paneTemplates.normalizePaneTemplates([
      { id: "", name: "empty", root: nestedShape },
    ]),
    applied: paneTemplates.applyTemplateToGroups(
      [
        {
          id: "g1",
          root: {
            type: "split",
            orientation: "vertical",
            sizes: [1 / 3, 1 / 3, 1 / 3],
            children: [tabGroups.paneLeaf("a"), tabGroups.paneLeaf("b"), tabGroups.paneLeaf("c")],
          },
          activeTabId: "c",
        },
      ],
      "c",
      {
        type: "split",
        orientation: "horizontal",
        sizes: [0.5, 0.5],
        children: [{ type: "pane" }, { type: "pane" }],
      },
      [],
      () => "new"
    ),
  },
  tabNavigation: {
    sibling: {
      forwardWrap: tabNavigation.getSiblingTabId(["t1", "t2", "t3"], "t3", 1),
      negativeOverflow: capture(() =>
        tabNavigation.getSiblingTabId(["t1", "t2", "t3"], "t1", -4)
      ),
      emptyActive: tabNavigation.getSiblingTabId(["t1"], "", 1) ?? null,
    },
    position: {
      ninthEmpty: tabNavigation.getTabIdAtPosition([], 9) ?? null,
      ninthMany: tabNavigation.getTabIdAtPosition(
        Array.from({ length: 12 }, (_, index) => `t${index + 1}`),
        9
      ),
      fractional: tabNavigation.getTabIdAtPosition(["a", "b"], 1.5) ?? null,
    },
    shortcut: {
      empty: tabNavigation.resolveTabShortcutTarget("", "t1"),
      missing: tabNavigation.resolveTabShortcutTarget(undefined, "t1"),
    },
    compare: {
      leadingZeros: tabNavigation.compareTabIds("tab-001", "tab-1"),
      hugeDifference: tabNavigation.compareTabIds("tab-999999999999999999999", "tab-2"),
      bothHuge: tabNavigation.compareTabIds(
        "tab-999999999999999999999",
        "tab-999999999999999999998"
      ),
      text: tabNavigation.compareTabIds("a", "b"),
    },
    sortedLeadingZeros: ["tab-10", "tab-001", "tab-2"].sort(tabNavigation.compareTabIds),
  },
  tabClosing: {
    unknown: tabClosing.getTabIdsForCloseAction(["a"], "missing", "close-others"),
    context: tabClosing.getTabCloseContext(["a", "b"], "a"),
    unknownContext: tabClosing.getTabCloseContext(["a"], "missing"),
    menu: tabClosing.getTabCloseMenuActions(
      ["a"],
      "a",
      {
        closeTab: "Close",
        closeOtherTabs: "Close others",
        closeTabsToLeft: "Close left",
        closeTabsToRight: "Close right",
      },
      true
    ),
    unknownMenu: tabClosing.getTabCloseMenuActions(
      ["a"],
      "missing",
      {
        closeTab: "Close",
        closeOtherTabs: "Close others",
        closeTabsToLeft: "Close left",
        closeTabsToRight: "Close right",
      },
      false
    ),
  },
  tabMenuActions: {
    splits: tabMenuActions.getTabSplitMenuActions({ type: "serial" }, t),
    page: tabMenuActions.getTabSplitMenuActions({ type: "settings" }, t),
    pinMissing: tabMenuActions.getTabPinMenuAction({ type: "ssh" }, t),
    pinTrue: tabMenuActions.getTabPinMenuAction(
      { type: "ssh", connectionHeaderPinned: true },
      t
    ),
    connectionSerialProfile: tabMenuActions.getTabConnectionMenuAction(
      { type: "serial", connection: { profileId: "p" } },
      t
    ),
    connectionEmptyProfile: tabMenuActions.getTabConnectionMenuAction(
      { type: "terminal", connection: { profileId: "" } },
      t
    ),
    renameSerial: tabMenuActions.getTabRenameMenuAction({ type: "serial" }, t),
    reopen: tabMenuActions.getReopenClosedTabMenuAction(t),
    ungroup: tabMenuActions.getUngroupMenuAction(t),
    layoutTemplates: tabMenuActions.getLayoutTemplatesMenuAction(t),
    panePage: tabMenuActions.getPaneMenuActions(
      { id: "a", title: "Logs", type: "logs", isActive: true },
      { isZoomed: true },
      t
    ),
    paneTerminal: tabMenuActions.getPaneMenuActions(terminal, { isZoomed: false }, t),
    paneTerminalRecording,
    logMenuRecording,
    logMenuRecordingSettings,
    logMenuIdle,
  },
  tabAttention: {
    totals,
    markListeners,
    stoppedListeners,
    commandScheduledDelay,
    commandImmediately,
    commandAfterDelay,
    commandAfterClear,
    commandEndedImmediately,
    replacementPendingTimers,
    replacementBeforeDelay,
    replacementAfterDelay,
    replacementAfterClear,
    immediateTimersAfterClear,
    clearBefore,
    clearAfter,
    passwordWaiting,
    passwordCleared,
    agentQuery,
  },
  tabAgentIndicator: {
    defaults: tabAgentIndicator.normalizeTabAgentIndicator(undefined),
    array: tabAgentIndicator.normalizeTabAgentIndicator([]),
    unknown: tabAgentIndicator.normalizeTabAgentIndicator({ working: "spin", waiting: "spin" }),
    upper: tabAgentIndicator.normalizeTabAgentIndicator({
      working_color: "#ABCDEF",
      waiting_color: "#abcdeg",
    }),
    unicode: tabAgentIndicator.normalizeTabAgentIndicator({
      working_color: "#ＡＢＣＤＥＦ",
      waiting_color: "#1234567",
    }),
    configJson: JSON.stringify(
      tabAgentIndicator.normalizeTabAgentIndicator({
        working: "braille",
        waiting_color: "#FF0000",
      })
    ),
  },
  appNavigation: {
    opened,
    notified: notified.length,
    firstRequest,
    secondRequest,
    firstHandler,
    secondHandler,
    replacedRequest,
    noHandlerRequest,
    noHandlerOpened,
  },
  tabNudge: {
    named: nudgeNamed,
    childEnd: nudgeChildEnd,
    ownEnd: nudgeOwnEnd,
    replayRunning: nudgeReplayRunning,
    replayEnded: nudgeReplayEnded,
    replayAgain: nudgeReplayAgain,
    missing: nudgeMissing,
    barRunning: nudgeBarRunning,
    barEnded: nudgeBarEnded,
    barAbsent: nudgeBarAbsent,
  },
  persistence: {
    sessionLayoutJson: JSON.stringify(saved),
    savedTabGroups: saved,
    treeJson: JSON.stringify(tabGroups.normalizePaneTree(tree)),
    shapeJson: JSON.stringify(paneTemplates.shapeOfTree(tabGroups.normalizePaneTree(tree))),
    nestedShapeJson: JSON.stringify(paneTemplates.normalizeLayoutShape(nestedShape)),
    templateJson: JSON.stringify(paneTemplates.normalizePaneTemplates([
      { id: "t1", name: " Ops ", root: nestedShape },
    ])),
  },
}

console.log(JSON.stringify(result, null, 2))
