// Runs the real TypeScript modules this card ports, without changing src/.
// The Tauri imports are inert stubs: only the synchronous title helpers are
// observed, and the async shell discovery is not part of the port.
import fs from "node:fs"
import ts from "typescript"

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

const tauriCoreUrl = dataUrl(`
export async function invoke() { return null }
`)
const tauriPathUrl = dataUrl(`
export async function homeDir() { return "/home/me" }
`)
const closedTabsUrl = dataUrl(`
export function placeReopenedTabs(groups) { return groups }
export function reopenedActiveTabId(layout, ids) { return ids[0] }
`)

const tabTypesUrl = moduleUrl("src/types/tab.ts")
const brandLogosUrl = moduleUrl("src/lib/brandLogos.ts")
const distroUrl = moduleUrl("src/lib/distro.ts")
const tabIconsUrl = moduleUrl("src/lib/tabIcons.ts", {
  "@/lib/brandLogos": brandLogosUrl,
  "@/lib/distro": distroUrl,
  "@/types/tab": tabTypesUrl,
})
const localTabTitleUrl = moduleUrl("src/lib/localTabTitle.ts", {
  "@tauri-apps/api/core": tauriCoreUrl,
  "@tauri-apps/api/path": tauriPathUrl,
  "@/lib/distro": distroUrl,
  "@/lib/tabIcons": tabIconsUrl,
  "@/types/tab": tabTypesUrl,
})
const tabGroupsUrl = moduleUrl("src/lib/tabGroups.ts")
const paneTemplatesUrl = moduleUrl("src/lib/paneTemplates.ts", {
  "@/lib/tabGroups": tabGroupsUrl,
})
const persistenceUrl = moduleUrl("src/lib/tabGroupsPersistence.ts", {
  "@/lib/tabGroups": tabGroupsUrl,
})
const tabsStateUrl = moduleUrl("src/lib/tabsState.ts", {
  "@/lib/localTabTitle": localTabTitleUrl,
  "@/lib/tabGroups": tabGroupsUrl,
  "@/lib/tabGroupsPersistence": persistenceUrl,
  "@/lib/paneTemplates": paneTemplatesUrl,
  "@/lib/closedTabs": closedTabsUrl,
  "@/types/tab": tabTypesUrl,
})

const tabGroups = await import(tabGroupsUrl)
const persistence = await import(persistenceUrl)
const tabIcons = await import(tabIconsUrl)
const localTabTitle = await import(localTabTitleUrl)
const tabsState = await import(tabsStateUrl)
const brandLogos = await import(brandLogosUrl)
const distro = await import(distroUrl)

const output = {}

const pane = (tabId) => ({ type: "pane", tabId })
const split = (orientation, children, sizes = children.map(() => 1 / children.length)) => ({
  type: "split",
  orientation,
  children,
  sizes,
})
const row = (children, sizes) => split("horizontal", children, sizes)
const column = (children, sizes) => split("vertical", children, sizes)
const group = (id, root, activeTabId = tabGroups.getPaneIds(root)[0], zoomedTabId) => ({
  id,
  root,
  activeTabId,
  ...(zoomedTabId === undefined ? {} : { zoomedTabId }),
})

const tree = row([pane("a"), column([pane("b"), pane("c")])])
const groupFixtures = [
  group("g1", row([pane("a"), pane("b")]), "b"),
  group("g2", pane("c")),
]

output.tabGroups = {
  paneIds: tabGroups.getPaneIds(row([pane("a"), column([pane("b"), pane("a")])])),
  paneIdsEmptySplit: tabGroups.getPaneIds({
    type: "split",
    orientation: "horizontal",
    children: [],
    sizes: [],
  }),
  paneLeaf: tabGroups.paneLeaf("a"),
  paneLeafEmpty: tabGroups.paneLeaf(""),
  splitOrientations: ["left", "right", "above", "below"].map((direction) => ({
    direction,
    orientation: tabGroups.splitOrientation(direction),
  })),
  foundGroup: tabGroups.findGroupOfTab(groupFixtures, "b")?.id ?? null,
  missingGroup: tabGroups.findGroupOfTab(groupFixtures, "x")?.id ?? null,
  emptyIdGroup: tabGroups.findGroupOfTab([group("g1", pane(""))], "")?.id ?? null,
  normalizeEmptySplit: tabGroups.normalizePaneTree({
    type: "split",
    orientation: "horizontal",
    children: [],
    sizes: [],
  }),
  insertMissingTarget: tabGroups.insertNextTo(row([pane("a"), pane("b")]), "x", pane("c"), "left"),
  insertEmptyId: tabGroups.insertNextTo(pane(""), "", pane("x"), "below"),
  removeNone: tabGroups.removePanes(tree, []),
  removeAll: tabGroups.removePanes(tree, ["a", "b", "c"]),
  removeDuplicate: tabGroups.removePanes(row([pane("a"), pane("a"), pane("b")]), ["a"]),
  resizeMissingPath: tabGroups.resizeSplit(tree, [9, 0], [0.5, 0.5]),
  resizeNegativePath: tabGroups.resizeSplit(tree, [-1], [0.5, 0.5]),
  resizeWrongCount: tabGroups.resizeSplit(tree, [], [1]),
  removeGroupsMissing: tabGroups.removeTabsFromGroups(groupFixtures, ["x"]),
  removeGroupsEmptyZoom: tabGroups.removeTabsFromGroups(
    [group("g1", row([pane("a"), pane("b")]), "a", "")],
    ["b"]
  ),
  fallbackEmptyActive: tabGroups.getFallbackTabId(groupFixtures, groupFixtures, "") ?? null,
  fallbackNullActive: tabGroups.getFallbackTabId(groupFixtures, groupFixtures, null) ?? null,
  fallbackNoNext: tabGroups.getFallbackTabId(groupFixtures, [], "a"),
  atomicNoMissing: tabGroups.reconcileGroups([group("g1", pane("a"))], ["a"], () => "new"),
  reconcileMissing: tabGroups.reconcileGroups(
    [group("g1", row([pane("a"), pane("gone")]))],
    ["a", "b"],
    () => "new-1"
  ),
  reconcileDuplicate: tabGroups.reconcileGroups(
    [group("g1", pane("a")), group("g2", pane("a"))],
    ["a"],
    () => "new"
  ),
  focusUnknown: tabGroups.focusPaneInGroups(groupFixtures, "x"),
  placeSelf: tabGroups.placePane(groupFixtures, "a", "a", "right"),
  placeMissing: tabGroups.placePane(groupFixtures, "a", "x", "right"),
  mergeSame: tabGroups.mergeGroup(groupFixtures, "a", "b", "right"),
  mergeMissing: tabGroups.mergeGroup(groupFixtures, "a", "x", "right"),
  breakNegative: tabGroups
    .breakOutPane([group("g1", row([pane("a"), pane("b")])), group("g2", pane("c"))], "b", "g3", -4)
    .map((candidate) => candidate.id),
  breakBeyond: tabGroups
    .breakOutPane([group("g1", row([pane("a"), pane("b")])), group("g2", pane("c"))], "b", "g3", 9)
    .map((candidate) => candidate.id),
  breakLone: tabGroups.breakOutPane([group("g1", pane("a"))], "a", "g2"),
  ungroupActiveMissing: tabGroups.ungroupPanes(
    [group("g1", row([pane("a"), pane("b")]), "x")],
    "a",
    () => "new"
  ),
  zoomEmptyId: tabGroups.toggleZoom([group("g1", row([pane(""), pane("b")]))], ""),
  visibleEmptyActive: tabGroups.getVisiblePaneIds([group("g1", row([pane(""), pane("b")]))], ""),
  visibleEmptyZoom: tabGroups.getVisiblePaneIds(
    [group("g1", row([pane(""), pane("b")]), "", "")],
    ""
  ),
  visibleEmptyZoomWithActive: tabGroups.getVisiblePaneIds(
    [group("g1", row([pane("a"), pane("b")]), "a", "")],
    "a"
  ),
  moveNegative: tabGroups.moveGroup(groupFixtures, -1, 0),
  moveBeyond: tabGroups.moveGroup(groupFixtures, 0, 2),
}

const plainTabIconConfig = () => ({ enabled: true, agents: true, shells: {} })
const styleObject = (fields) => ({ ...fields })
const configWithShells = (shells) => ({ enabled: true, agents: true, shells })

output.tabIcons = {
  constants: {
    ids: tabIcons.TAB_ICON_IDS,
    agents: tabIcons.AGENT_LOGO_IDS,
    agentColors: tabIcons.AGENT_LOGO_COLORS,
    colors: tabIcons.TAB_ICON_COLORS,
    defaultConfig: tabIcons.DEFAULT_TAB_ICON_CONFIG,
  },
  shellKeys: [
    " PWSH\u3000",
    "\uFEFFZsh\u2003",
    "\u00A0bash\u00A0",
    "Σ",
    "ΟΣ",
    "ΣΣ",
    "İ",
    "ẞ",
    "Git Bash",
    "",
    "   ",
    null,
    undefined,
  ].map((input) => ({ input, key: tabIcons.shellKey(input ?? undefined) })),
  normalizedStyles: [
    { icon: "fish", color: "#AABBCC" },
    { icon: "unicorn", color: "red" },
    { color: "" },
    { color: "#abcdef" },
    { color: "#ABCDEF0" },
    { color: "#GGGGGG" },
    { color: "＃AABBCC" },
    { icon: "rocket", color: "#ABCDEF" },
    { icon: "distro:arch" },
    { icon: "distro:windows" },
    { icon: "logo:docker" },
    { icon: "logo:fish" },
    { icon: "agent:claude" },
    { icon: "agent:my-agent" },
    { icon: "agent:" },
    { icon: "distro:" },
    { icon: "logo:" },
    { icon: "ROCKET" },
    "pwsh",
    null,
    [],
    ...brandLogos.BRAND_LOGO_IDS.map((icon) => ({ icon: `logo:${icon}` })),
    ...distro.DISTRO_IDS.map((icon) => ({ icon: `distro:${icon}` })),
    ...tabIcons.TAB_ICON_IDS.map((icon) => ({ icon })),
  ].map((input) => ({ input, style: tabIcons.normalizeTabIconStyle(input) ?? null })),
  normalizedConfigs: [
    undefined,
    null,
    [],
    {
      enabled: false,
      agents: false,
      shells: { "Git Bash": { icon: "box" }, cmd: { icon: "nope" } },
    },
    {
      enabled: 0,
      agents: "false",
      shells: {
        "  PWSH ": { icon: "box" },
        "": { icon: "rocket" },
        "   ": { icon: "rocket" },
        "zsh": { color: "#AABBCC" },
        "cmd": { color: "#GGGGGG" },
        "wsl": { icon: "distro:windows" },
        "nu": { icon: "logo:fish" },
        "fish": { icon: "agent:claude" },
        "elvish": { color: "#abcdef", icon: "rocket" },
      },
    },
    { shells: [{ icon: "star" }, "nope"] },
  ].map((input) => ({ input, config: tabIcons.normalizeTabIconConfig(input) })),
  shellsForSettings: [
    {
      platform: "macos",
      config: configWithShells({ nu: { icon: "box" }, zsh: { icon: "star" } }),
      found: [],
    },
    {
      platform: "windows",
      config: plainTabIconConfig(),
      found: [],
    },
    {
      platform: "linux",
      config: plainTabIconConfig(),
      found: ["Nu", null, " zsh ", "Σ", "", " zsh "],
    },
    {
      platform: "windows",
      config: configWithShells({ nu: { icon: "box" }, elvish: { icon: "star" } }),
      found: ["nu"],
    },
    {
      platform: "linux",
      config: configWithShells({
        "b": {},
        "A": {},
        "ä": {},
        "Σ": {},
        "🔧": {},
        "aa": {},
        "Z": {},
      }),
      found: [],
    },
  ].map(({ platform, config, found }) => ({
    platform,
    config,
    found,
    shells: tabIcons.shellsForSettings(platform, config, found),
  })),
  resolved: [
    { tab: { type: "terminal", shellName: "pwsh" } },
    { tab: { type: "terminal", shellName: " PWSH\u3000" } },
    { tab: { type: "terminal", shellName: "elvish" } },
    { tab: { type: "terminal", shellName: "Σ" } },
    {
      tab: { type: "terminal", shellName: "pwsh", iconStyle: { icon: "rocket", color: "" } },
      config: configWithShells({ pwsh: { color: "#e5534b" } }),
    },
    { tab: { type: "ssh" } },
    { tab: { type: "ssh", distroId: "debian" } },
    {
      tab: { type: "ssh", distroId: "ubuntu", iconStyle: { icon: "database", color: "#e5534b" } },
    },
    { tab: { type: "terminal", shellName: "WSL" } },
    { tab: { type: "terminal", shellName: " WSL ", distroId: "arch" } },
    {
      tab: { type: "terminal", shellName: "WSL", distroId: "arch" },
      config: configWithShells({ wsl: { icon: "rocket" } }),
    },
    { tab: { type: "terminal", shellName: "pwsh", distroId: "arch" } },
    { tab: { type: "remote-file-editor" } },
    { tab: { type: "terminal", shellName: "pwsh" }, agent: "claude" },
    { tab: { type: "ssh", distroId: "debian" }, agent: "pi" },
    { tab: { type: "ssh" }, agent: "codex" },
    { tab: { type: "terminal", shellName: "pwsh" }, agent: "my-agent" },
    { tab: { type: "terminal", shellName: "pwsh" }, config: { enabled: true, agents: false, shells: {} }, agent: "claude" },
    {
      tab: { type: "terminal", shellName: "pwsh", iconStyle: { icon: "agent:claude" } },
      config: { enabled: true, agents: false, shells: {} },
    },
    {
      tab: { type: "terminal", shellName: "pwsh", iconStyle: { icon: "agent:claude" } },
      agent: "codex",
    },
  ].map(({ tab, config = plainTabIconConfig(), agent }) => ({
    tab,
    config,
    agent,
    icon: tabIcons.resolveTabIcon(tab, config, agent),
  })),
  pickedColors: [
    "logo:docker",
    "logo:github",
    "agent:claude",
    "agent:codex",
    "rocket",
    "distro:arch",
  ].map((icon) => ({ icon, color: tabIcons.pickedLogoColor(icon) ?? null })),
  badges: [
    { tab: { type: "terminal", shellName: "WSL" }, icon: { icon: "distro:ubuntu", color: "" } },
    { tab: { type: "terminal", shellName: " WSL " }, icon: { icon: "distro:arch", color: "" } },
    { tab: { type: "terminal", shellName: "WSL" }, icon: { icon: "rocket", color: "" } },
    { tab: { type: "ssh" }, icon: { icon: "distro:ubuntu", color: "" } },
    { tab: { type: "terminal", shellName: "zsh" }, icon: { icon: "distro:ubuntu", color: "" } },
  ].map(({ tab, icon }) => ({ tab, icon, badge: tabIcons.hasLocalBadge(tab, icon) })),
  localDistro: (() => {
    const local = { id: "a", title: "WSL", type: "terminal", isActive: true }
    const next = tabIcons.withLocalDistro(local, "debian")
    return {
      tab: local,
      next,
      again: tabIcons.withLocalDistro(next, "debian"),
      sameAfter: next === tabIcons.withLocalDistro(next, "debian"),
      ssh: tabIcons.withLocalDistro({ ...local, type: "ssh" }, "debian"),
    }
  })(),
  normalizedSsh: tabIcons.normalizeSavedSshTab({
    id: "a",
    title: "host",
    type: "ssh",
    isActive: false,
    distroId: "beos",
    iconStyle: { icon: "unicorn" },
  }),
  withShellStyles: (() => {
    const set = tabIcons.withShellIconStyle(plainTabIconConfig(), "zsh", { icon: "star" })
    const spaced = tabIcons.withShellIconStyle(plainTabIconConfig(), "  ZSH ", { icon: "star" })
    return {
      set,
      cleared: tabIcons.withShellIconStyle(set, "zsh", undefined),
      invalid: tabIcons.withShellIconStyle(set, "zsh", { icon: "unicorn" }),
      spaced,
      emptyKey: tabIcons.withShellIconStyle(plainTabIconConfig(), "", { icon: "star" }),
      renumbered: tabIcons.withShellIconStyle(
        configWithShells({ a: { icon: "box" } }),
        "1",
        { icon: "star" }
      ),
      renumberedBytes: JSON.stringify(
        tabIcons.withShellIconStyle(configWithShells({ a: { icon: "box" } }), "1", {
          icon: "star",
        })
      ),
    }
  })(),
  shellTabIcons: ["Git Bash", "nu", "bash", " zsh\u3000", null].map((shellName) => ({
    shellName,
    icon: tabIcons.shellTabIcon(
      configWithShells({ bash: { color: "#a371f7" } }),
      shellName ?? undefined
    ),
  })),
  serializedConfig: JSON.stringify(
    configWithShells({
      "git bash": { icon: "box" },
      zsh: { color: "#aabbcc" },
      elvish: { icon: "logo:docker", color: "" },
    })
  ),
  serializedNormalizedConfig: (() => {
    const config = tabIcons.normalizeTabIconConfig({
      enabled: true,
      agents: true,
      shells: {
        "  PWSH ": { icon: "box" },
        "10": { icon: "rocket" },
        "2": { icon: "star" },
        "1": { color: "#AABBCC" },
        zsh: { icon: "logo:docker", color: "" },
        "": { icon: "star" },
      },
    })
    return { config, bytes: JSON.stringify(config) }
  })(),
  serializedRenumberedShells: (() => {
    const input = {
      enabled: true,
      agents: true,
      shells: {
        a: { icon: "box" },
        " 1 ": { icon: "rocket" },
        " 2 ": { icon: "star" },
        b: { icon: "fish" },
      },
    }
    const config = tabIcons.normalizeTabIconConfig(input)
    return { input, config, bytes: JSON.stringify(config) }
  })(),
  serializedStyle: JSON.stringify(
    tabIcons.normalizeTabIconStyle({ icon: "rocket", color: "#ABCDEF" })
  ),
  serializedDefaultConfig: JSON.stringify(tabIcons.DEFAULT_TAB_ICON_CONFIG),
  serializedStyles: [
    { icon: "rocket", color: "#ABCDEF" },
    { color: "" },
    { icon: "distro:arch" },
  ].map((input) => ({
    input,
    style: tabIcons.normalizeTabIconStyle(input),
    bytes: JSON.stringify(tabIcons.normalizeTabIconStyle(input)),
  })),
  displayNames: ["pwsh", "powershell", "cmd", "git bash", "wsl", "nu", "elvish", ""].map(
    (key) => ({ key, name: tabIcons.shellDisplayName(key) })
  ),
  builtInIcons: ["pwsh", "powershell", "cmd", "git bash", "wsl", "bash", "zsh", "sh", "fish", "nu", "elvish", ""].map(
    (key) => ({ key, icon: tabIcons.builtInShellIcon(key) })
  ),
  prefixes: [
    { icon: "logo:docker" },
    { icon: "logo:fish" },
    { icon: "agent:claude" },
    { icon: "agent:nope" },
    { icon: "distro:arch" },
    { icon: "rocket" },
  ].map(({ icon }) => ({
    icon,
    brand: tabIcons.isBrandTabIcon(icon) ? tabIcons.tabIconBrand(icon) : null,
    agent: tabIcons.isAgentTabIcon(icon) ? tabIcons.tabIconAgent(icon) : null,
    distro: tabIcons.isDistroTabIcon(icon) ? tabIcons.tabIconDistro(icon) : null,
    isBrand: tabIcons.isBrandTabIcon(icon),
    isAgent: tabIcons.isAgentTabIcon(icon),
    isDistro: tabIcons.isDistroTabIcon(icon),
  })),
  agentIcons: ["claude", "codex", "opencode", "pi", "my-agent", ""].map((agent) => ({
    agent,
    icon: tabIcons.agentTabIcon(agent),
  })),
  sshIcons: [null, "debian", "arch"].map((distro) => ({
    distro,
    icon: tabIcons.sshTabIcon(distro ?? undefined),
  })),
  inherited: [
    { type: "ssh", distroId: "debian" },
    { type: "ssh" },
    { type: "serial", distroId: "arch" },
    { type: "terminal", shellName: "pwsh", distroId: "arch" },
    { type: "terminal", shellName: "WSL", distroId: "arch" },
    { type: "terminal", shellName: "WSL" },
    { type: "terminal", shellName: "zsh" },
  ].map((tab) => ({
    tab,
    icon: tabIcons.inheritedTabIcon(tab, plainTabIconConfig()),
  })),
  hasIcons: [
    { type: "terminal" },
    { type: "ssh" },
    { type: "serial" },
    { type: "settings" },
    { type: "remote-file-editor" },
  ].map((tab) => ({ tab, has: tabIcons.hasTabIcon(tab) })),
  appliedStyles: [
    { base: { icon: "terminal", color: "" }, style: undefined },
    { base: { icon: "terminal", color: "" }, style: {} },
    { base: { icon: "terminal", color: "" }, style: { icon: "star" } },
    { base: { icon: "terminal", color: "" }, style: { color: "" } },
    { base: { icon: "terminal", color: "#123456" }, style: { color: "#abcdef" } },
  ].map(({ base, style }) => ({
    base,
    style,
    icon: tabIcons.applyTabIconStyle(base, style),
  })),
}

const localTab = (id, fields = {}) => ({
  id,
  title: id,
  type: "terminal",
  isActive: false,
  ...fields,
})
const connection = (fields = {}) => ({ type: "terminal", ...fields })
const mapOf = (entries) => new Map(Object.entries(entries))

output.localTabTitle = {
  directories: [
    { cwd: "", home: null },
    { cwd: "D:\\code\\rust\\tTerm", home: null },
    { cwd: "/home/me/src/", home: null },
    { cwd: "\\\\server\\share", home: null },
    { cwd: "C:\\Users\\Me", home: "C:\\Users\\Me" },
    { cwd: "c:/users/me/", home: "C:\\Users\\Me" },
    { cwd: "/c/Users/Me", home: "C:\\Users\\Me" },
    { cwd: "/home/me/src", home: "/home/me" },
    { cwd: "/", home: "/home/me" },
    { cwd: "C:\\", home: null },
    { cwd: "C:", home: null },
    { cwd: "  ", home: null },
    { cwd: "\u3000/src/\uFEFF", home: null },
    { cwd: "/工作/项目/", home: null },
    { cwd: "/a/😀", home: null },
    { cwd: "/a/e\u0301", home: null },
    { cwd: "C:\\用户\\项目\\", home: null },
    { cwd: "/c", home: null },
    { cwd: "1:/x", home: null },
    { cwd: "\u00A0/home/me/\u00A0", home: "/home/me" },
    { cwd: "/home/me", home: "" },
    { cwd: "C:\\ΟΣ", home: "c:\\ος" },
    { cwd: "C:\\İ", home: "c:\\i\u0307" },
  ].map(({ cwd, home }) => ({
    cwd,
    home,
    title: localTabTitle.directoryTitle(cwd, home),
  })),
  commands: [
    "",
    "npm run dev",
    "  ssh prod  ",
    "cargo test --all-targets -- -D warnings",
    "C:\\tools\\vim.exe notes.txt",
    '& "C:\\Program Files\\node.exe" app.js',
    "& .\\build.ps1",
    "RUST_LOG=debug sudo /usr/bin/htop",
    "sudo RUST_LOG=debug /usr/bin/htop",
    "变量=值 cmd",
    "echo 😀😀😀😀😀😀😀",
    "echo 😀😀😀😀😀😀😀😀",
    "echo 汉字汉字汉字汉字汉字汉字",
    "echo e\u0301e\u0301e\u0301e\u0301e\u0301e\u0301e\u0301",
    "echo\nrm -rf /",
    "a\n/b",
    "\"a b\"",
    "   ",
    "sudo",
    "& ",
    "& echo hi",
    "FOO=1 bar",
    "FOO='two words' echo",
    "FOO= echo",
    "PATH=C:\\a\\b.exe run",
    "12345678901234567890",
    "echo 123456789012345",
    "echo 1234567890123456",
    "say \"hello world\"",
    "say 'hello world'",
    "/usr/bin/",
    "ÉCHO ok",
  ].map((command) => ({ command, title: localTabTitle.commandTitle(command) })),
  running: (() => {
    const tabs = [
      localTab("1", { title: "tTerm", autoTitle: true }),
      localTab("2", { title: "web/src", autoTitle: true }),
      localTab("3", { title: "api", autoTitle: true }),
      localTab("4", { title: "named", autoTitle: undefined }),
      localTab("5", { title: "empty", autoTitle: true }),
      localTab("6", { title: "标签", autoTitle: true }),
      localTab("7", { title: "另一", autoTitle: true }),
    ]
    const commands = mapOf({
      "1": "claude",
      "2": "claude --dangerously-skip-permissions",
      "3": "codex",
      "4": "claude",
      "5": "",
      "6": "echo 😀😀😀😀😀😀😀😀",
      "7": "echo",
    })
    return tabs.map((tab) => ({
      tab,
      title: localTabTitle.runningCommandTitle(tab, commands),
    }))
  })(),
  localShellTitles: [
    { shellName: "WSL", connection: connection({ terminalShell: "wsl", terminalShellWslDistro: "Debian" }) },
    { shellName: "WSL", connection: connection({ terminalShell: "wsl", terminalShellWslDistro: " \u3000Debian\u00A0 " }) },
    { shellName: "WSL", connection: connection({ terminalShell: "wsl", terminalShellWslDistro: "   " }) },
    { shellName: "WSL", connection: connection({ terminalShell: "wsl" }) },
    { shellName: "pwsh", connection: connection({ terminalShell: "pwsh", terminalShellWslDistro: "Debian" }) },
    { shellName: "WSL", connection: connection({ terminalShell: "custom", terminalShellWslDistro: "Debian" }) },
    { shellName: undefined, connection: undefined },
    { shellName: "", connection: connection() },
    { shellName: "zsh", connection: null },
  ].map(({ shellName, connection }) => ({
    shellName,
    connection,
    title: localTabTitle.localShellTitle(shellName, connection) ?? null,
  })),
  cwds: [
    { tab: localTab("1", { autoTitle: true }), cwd: "/work/tTerm" },
    { tab: localTab("1", { title: "builds" }), cwd: "/work/tTerm" },
    {
      tab: localTab("1", {
        title: "tTerm",
        autoTitle: true,
        connection: connection({ cwd: "/work/tTerm" }),
      }),
      cwd: "/work/tTerm",
    },
    {
      tab: localTab("1", { title: "", autoTitle: true }),
      cwd: "",
    },
    {
      tab: localTab("1", { title: "terminal", autoTitle: true, connection: undefined }),
      cwd: "/x",
    },
    {
      tab: localTab("1", { title: "ssh", autoTitle: true, connection: { type: "ssh" } }),
      cwd: "/x",
    },
  ].map(({ tab, cwd }) => ({
    tab,
    cwd,
    next: localTabTitle.withLocalCwd(tab, cwd),
    same: localTabTitle.withLocalCwd(tab, cwd) === tab,
  })),
  tooltips: [
    localTab("1", { title: "builds", connection: connection({ cwd: "/work" }) }),
    localTab("1", { title: "pwsh", connection: connection() }),
    localTab("1", { title: "prod", type: "ssh", connection: { host: "10.0.0.1" } }),
    localTab("1", {
      title: "tTerm",
      shellName: "pwsh",
      connection: connection({ cwd: "D:\\code\\tTerm" }),
    }),
    localTab("1", {
      title: "~",
      shellName: "WSL",
      connection: connection({
        cwd: "/home/me",
        terminalShell: "wsl",
        terminalShellWslDistro: "Debian",
      }),
    }),
    localTab("1", {
      title: "same",
      connection: connection({ cwd: "same" }),
    }),
    localTab("1", {
      title: "host",
      type: "serial",
      connection: { host: "COM3" },
    }),
    localTab("1", { title: "no connection" }),
  ].map((tab) => ({ tab, tooltip: localTabTitle.tabTooltip(tab) })),
  distinct: (() => {
    const local = (id, cwd, fields = {}) =>
      localTab(id, {
        title: "",
        autoTitle: true,
        connection: connection({ cwd }),
        ...fields,
      })
    const cases = [
      {
        tabs: [local("1", "/work/web/src"), local("2", "/work/api/src"), local("3", "/home/me/api/src")],
        home: null,
      },
      {
        tabs: [local("1", "D:\\code\\tTerm"), local("2", "d:/code/tTerm/")],
        home: null,
      },
      {
        tabs: [local("1", "C:\\a\\src"), local("2", "C:\\b\\src")],
        home: null,
      },
      {
        tabs: [local("1", "/src"), local("2", "/a/src")],
        home: null,
      },
      {
        tabs: [
          local("1", "/a/src", { title: "src" }),
          local("2", "/b/src", { autoTitle: undefined, title: "mine" }),
        ],
        home: null,
      },
      {
        tabs: [local("1", "/home/me")],
        home: "/home/me",
      },
      {
        tabs: [local("1", "/工作/项目/src"), local("2", "/其它/项目/src")],
        home: null,
      },
      {
        tabs: [local("1", "/a/😀/src"), local("2", "/b/😀/src")],
        home: null,
      },
      {
        tabs: [local("same", "/a/src"), local("same", "/b/src")],
        home: null,
      },
      {
        tabs: [local("1", "/a/src", { autoTitle: undefined }), local("2", "/b/src")],
        home: null,
      },
      {
        tabs: [local("1", "/a/e\u0301/src"), local("2", "/b/e\u0301/src")],
        home: null,
      },
      {
        tabs: [local("1", "/a/src/"), local("2", "/b/src")],
        home: null,
      },
    ]
    return cases.map(({ tabs, home }) => {
      const next = localTabTitle.distinctLocalTitles(tabs, home)
      return {
        tabs,
        home,
        titles: next.map((tab) => tab.title),
        changed: next !== tabs,
      }
    })
  })(),
  withDistinct: (() => {
    const tabs = [
      localTab("1", {
        title: "src",
        autoTitle: true,
        connection: connection({ cwd: "/a/src" }),
      }),
    ]
    return localTabTitle.withDistinctLocalTitles(tabs).map((tab) => tab.title)
  })(),
  legacyTitles: [
    "OS terminal",
    "OS terminal-3",
    "默认终端",
    "本地终端",
    "终端-12",
    "Terminal",
    "Local terminal",
    "Terminal-０",
    "Terminal-3x",
    "builds",
  ].map((title) => ({
    title,
    terminal: localTabTitle.hasLegacyLocalTitle({ type: "terminal", title }),
    ssh: localTabTitle.hasLegacyLocalTitle({ type: "ssh", title }),
  })),
}

output.tabsState = {
  remove: (() => {
    const base = (id, fields = {}) => ({
      id,
      title: id,
      type: "terminal",
      isActive: false,
      ...fields,
    })
    const cases = [
      {
        name: "active removed first",
        tabs: [base("a", { isActive: true }), base("b"), base("c")],
        activeTabId: "a",
        ids: ["a"],
      },
      {
        name: "active removed in the middle",
        tabs: [base("a"), base("b", { isActive: true }), base("c")],
        activeTabId: "b",
        ids: ["b"],
      },
      {
        name: "active removed last",
        tabs: [base("a"), base("b"), base("c", { isActive: true })],
        activeTabId: "c",
        ids: ["c"],
      },
      {
        name: "unknown active",
        tabs: [base("a"), base("b")],
        activeTabId: "missing",
        ids: ["a"],
      },
      {
        name: "null active",
        tabs: [base("a"), base("b")],
        activeTabId: null,
        ids: ["a"],
      },
      {
        name: "nothing removed",
        tabs: [base("a", { isActive: true }), base("b")],
        activeTabId: "a",
        ids: ["x"],
      },
      {
        name: "empty ids",
        tabs: [base("a", { isActive: true })],
        activeTabId: "a",
        ids: [],
      },
      {
        name: "all removed",
        tabs: [base("a", { isActive: true })],
        activeTabId: "a",
        ids: ["a"],
      },
      {
        name: "preferred survives, current survives",
        tabs: [base("a", { isActive: true, hasConnected: false }), base("b", { hasConnected: true }), base("c")],
        activeTabId: "a",
        ids: ["c"],
        options: { preferredActiveTabId: "b" },
      },
      {
        name: "activate preferred",
        tabs: [base("a", { isActive: true }), base("b")],
        activeTabId: "a",
        ids: ["c"],
        options: { preferredActiveTabId: "b", activatePreferred: true },
      },
      {
        name: "preferred after current removed",
        tabs: [base("a", { isActive: true }), base("b")],
        activeTabId: "a",
        ids: ["a"],
        options: { preferredActiveTabId: "b" },
      },
      {
        name: "preferred also removed",
        tabs: [base("a", { isActive: true }), base("b"), base("c")],
        activeTabId: "a",
        ids: ["a", "b"],
        options: { preferredActiveTabId: "b" },
      },
      {
        name: "empty preferred id",
        tabs: [base("", { isActive: false }), base("b", { isActive: true })],
        activeTabId: "b",
        ids: ["b"],
        options: { preferredActiveTabId: "" },
      },
      {
        name: "empty active id matches a tab",
        tabs: [base("", { isActive: true }), base("b")],
        activeTabId: "",
        ids: ["x"],
      },
      {
        name: "removing the empty active id",
        tabs: [base(""), base("b", { isActive: true })],
        activeTabId: "",
        ids: [""],
      },
      {
        name: "duplicate removal ids",
        tabs: [base("a"), base("b", { isActive: true })],
        activeTabId: "b",
        ids: ["a", "a"],
      },
      {
        name: "inactive hasConnected stays undefined",
        tabs: [base("a", { isActive: true }), base("b")],
        activeTabId: "a",
        ids: ["a"],
      },
      {
        name: "inactive hasConnected false stays false",
        tabs: [base("a", { isActive: true }), base("b", { hasConnected: false })],
        activeTabId: "a",
        ids: ["a"],
      },
    ]
    return cases.map(({ name, tabs, activeTabId, ids, options }) => {
      const before = tabs
      const result = tabsState.removeTabsFromState(tabs, activeTabId, ids, options)
      return {
        name,
        tabs: before,
        activeTabId,
        ids,
        options: options ?? {},
        result: JSON.parse(JSON.stringify(result)),
        sameReference: result.tabs === before,
      }
    })
  })(),
  defaults: [
    { id: "a", title: "a", type: "terminal", isActive: true },
    { id: "a", title: "a", type: "terminal", isActive: false },
    { id: "a", title: "a", type: "terminal", isActive: false, hasConnected: false },
    { id: "a", title: "a", type: "terminal", isActive: false, hasConnected: true },
    { id: "a", title: "a", type: "terminal", isActive: false, sessionNonce: 7 },
    { id: "a", title: "a", type: "terminal", isActive: false, connectionHeaderPinned: false },
    {
      id: "a",
      title: "Settings",
      type: "settings",
      isActive: true,
      connection: { type: "terminal" },
      sessionNonce: 9,
    },
    { id: "a", title: "Tunnels", type: "tunnels", isActive: false, connection: { host: "x" } },
    { id: "a", title: "Logs", type: "logs", isActive: false },
    {
      id: "a",
      title: "file",
      type: "remote-file-editor",
      isActive: false,
      connection: { host: "x", username: "me" },
      sessionNonce: 3,
    },
    {
      id: "a",
      title: "ssh",
      type: "ssh",
      isActive: false,
      hasConnected: false,
      connectionHeaderPinned: false,
      sessionNonce: 0,
      connection: { host: "x" },
    },
  ].map((tab) => {
    const next = tabsState.ensureTabDefaults(tab)
    return {
      tab,
      next,
      fields: {
        hasConnected: next.hasConnected === undefined ? "__undefined__" : next.hasConnected,
        sessionNonce: next.sessionNonce === undefined ? "__undefined__" : next.sessionNonce,
        connectionHeaderPinned:
          next.connectionHeaderPinned === undefined
            ? "__undefined__"
            : next.connectionHeaderPinned,
        connection: next.connection === undefined ? "__undefined__" : next.connection,
      },
    }
  }),
  duplicateTitles: [
    { base: "web", existing: [] },
    { base: "web", existing: ["web-2"] },
    { base: "web", existing: ["web-2", "web-3", "web-4"] },
    { base: "web", existing: ["web-2", "web-4"] },
    { base: "", existing: ["-2"] },
    { base: "标签", existing: ["标签-2"] },
    { base: "web", existing: ["web-2", "web-2"] },
  ].map(({ base, existing }) => ({
    base,
    existing,
    title: tabsState.getDuplicateTabTitle(base, existing),
  })),
}

const savedLayouts = []
const addLayout = (name, layout, tabIds) => {
  const restored = persistence.restoreTabGroups(layout, tabIds)
  const withIds = restored.map((candidate, index) => ({
    ...candidate,
    id: `group-${index + 1}`,
  }))
  const serialized = persistence.serializeTabGroups(withIds, new Set(tabIds))
  savedLayouts.push({
    name,
    layout,
    tabIds,
    restored: JSON.parse(JSON.stringify(restored)),
    bytes: JSON.stringify(serialized),
  })
}

addLayout(
  "current simple with unsaved pane and zoom",
  {
    version: 2,
    groups: [
      {
        root: row([pane("a"), pane("settings")]),
        activeTabId: "settings",
        zoomedTabId: "settings",
        unknown: "ignored",
      },
      { root: pane("editor"), activeTabId: "editor" },
    ],
  },
  ["a", "editor"]
)
addLayout(
  "current with empty and duplicate ids",
  {
    version: 2,
    groups: [
      { root: row([pane(""), pane(""), pane("a")]), activeTabId: "", zoomedTabId: "" },
      { root: pane("a"), activeTabId: "missing", zoomedTabId: "missing" },
    ],
  },
  ["", "a"]
)
addLayout(
  "current with malformed groups and sizes",
  {
    version: 2,
    groups: [
      "nonsense",
      { root: { type: "split", orientation: "diagonal", children: [], sizes: [] } },
      { root: { type: "pane", tabId: 7 } },
      { root: { type: "pane", tabId: "a" }, activeTabId: "a" },
      {
        root: {
          type: "split",
          orientation: "horizontal",
          children: [pane("a"), pane("b"), pane("c")],
          sizes: [0.5],
        },
        activeTabId: "b",
      },
      {
        root: {
          type: "split",
          orientation: "vertical",
          children: [pane("c"), pane("a")],
          sizes: [0, -1, 2, 3],
        },
        activeTabId: "c",
      },
      {
        root: {
          type: "split",
          orientation: "horizontal",
          children: [pane("b")],
          sizes: [3],
        },
        activeTabId: "b",
      },
      {
        root: {
          type: "split",
          orientation: "horizontal",
          children: [],
          sizes: [],
        },
      },
    ],
  },
  ["a", "b", "c"]
)
addLayout(
  "current with full precision and exponent sizes",
  {
    version: 2,
    groups: [
      {
        root: row(
          [pane("a"), pane("b"), pane("c"), pane("d"), pane("e")],
          [1e-7, 0.3333333333333333, 1e21, 0.1, 17]
        ),
        activeTabId: "c",
        zoomedTabId: "a",
      },
    ],
  },
  ["a", "b", "c", "d", "e"]
)
addLayout("version 2.0", { version: 2.0, groups: [{ root: pane("a"), activeTabId: "a" }] }, ["a"])
addLayout("version string 2 is Dockview", { version: "2", groups: [{ root: pane("a") }] }, ["a"])
addLayout("old version is Dockview", { version: 1, groups: [{ root: pane("a") }] }, ["a"])
addLayout("missing version", { groups: [{ root: pane("a") }] }, ["a"])
addLayout("real old resume sample", { root: "tab-1" }, ["tab-1"])
addLayout("not a record", null, ["a"])
addLayout("array layout", [1, 2], ["a"])
addLayout("groups not array", { version: 2, groups: {} }, ["a"])
addLayout("grid not record", { grid: [] }, ["a"])
addLayout("grid empty", { grid: {} }, ["a"])
addLayout(
  "dockview from tests",
  {
    grid: {
      orientation: "HORIZONTAL",
      root: {
        type: "branch",
        data: [
          { type: "leaf", data: { id: "1", views: ["a", "b"], activeView: "b" }, size: 300 },
          {
            type: "branch",
            size: 700,
            data: [
              { type: "leaf", data: { id: "2", views: ["c"], activeView: "c" }, size: 100 },
              { type: "leaf", data: { id: "3", views: ["d"], activeView: "d" }, size: 300 },
            ],
          },
        ],
      },
    },
    panels: {},
    activeGroup: "3",
  },
  ["a", "b", "c", "d"]
)
addLayout(
  "dockview vertical, duplicates and odd views",
  {
    grid: {
      orientation: "VERTICAL",
      root: {
        type: "branch",
        data: [
          {
            type: "leaf",
            data: { id: "1", views: ["a", "a", "", "b"], activeView: "missing" },
            size: 0,
          },
          { type: "leaf", data: { id: "2", views: ["", "c"], activeView: "c" } },
          { type: "leaf", data: { id: "3", views: [], activeView: "a" } },
          { type: "leaf", data: { id: "4", views: [1, null, "d"] } },
          "nonsense",
          { type: "branch", data: "nonsense" },
          { type: "unknown", data: {} },
          { type: "leaf", data: { id: "5", views: ["e"], activeView: 3 }, size: -2 },
        ],
      },
    },
    activeGroup: "5",
  },
  ["a", "b", "c", "d", "e"]
)
addLayout(
  "dockview active group maps to behind view",
  {
    grid: {
      root: {
        type: "branch",
        data: [{ type: "leaf", data: { id: "x", views: ["a", "b"], activeView: "a" } }],
      },
    },
    activeGroup: "x",
  },
  ["a", "b"]
)
addLayout(
  "dockview empty active group id",
  {
    grid: {
      root: {
        type: "branch",
        data: [{ type: "leaf", data: { id: "", views: ["a"] } }],
      },
    },
    activeGroup: "",
  },
  ["a"]
)
addLayout(
  "dockview unknown panes and duplicate groups",
  {
    grid: {
      root: {
        type: "branch",
        data: [
          { type: "leaf", data: { id: "1", views: ["gone", "a"] } },
          { type: "leaf", data: { id: "1", views: ["b"] } },
        ],
      },
    },
    activeGroup: "1",
  },
  ["a", "b"]
)
addLayout(
  "dockview root leaf",
  { grid: { orientation: "VERTICAL", root: { type: "leaf", data: { views: ["b", "a"] } } } },
  ["a", "b"]
)
addLayout(
  "dockview branch with no valid children",
  { grid: { root: { type: "branch", data: [{ type: "leaf", data: { views: [] } }] } } },
  ["a"]
)
addLayout(
  "dockview unknown fields",
  {
    version: 7,
    unknown: { deep: [1, 2] },
    grid: {
      unknown: true,
      root: {
        type: "branch",
        data: [{ type: "leaf", data: { views: ["a"], activeView: "a", unknown: 1 }, size: 4, unknown: 2 }],
      },
    },
    panels: { a: { id: "a" } },
  },
  ["a"]
)

output.persistence = savedLayouts

const serializedGroups = []
const addSerializeLayout = (name, groups, savedTabIds) => {
  const serialized = persistence.serializeTabGroups(groups, new Set(savedTabIds))
  serializedGroups.push({
    name,
    groups,
    savedTabIds,
    value: JSON.parse(JSON.stringify(serialized)),
    bytes: JSON.stringify(serialized),
  })
}

addSerializeLayout(
  "drops unsaved panes and fixes the active and zoom panes",
  [
    {
      id: "g1",
      root: row([pane("a"), pane("settings"), pane("b")], [0.5, 0.25, 0.25]),
      activeTabId: "settings",
      zoomedTabId: "settings",
    },
    { id: "g2", root: pane("editor"), activeTabId: "editor" },
  ],
  ["a", "b"]
)
addSerializeLayout(
  "keeps empty strings out of optional fields",
  [
    {
      id: "g1",
      root: row([pane("a"), pane("b")], [0.5, 0.5]),
      activeTabId: "a",
      zoomedTabId: "",
    },
  ],
  ["a", "b"]
)
addSerializeLayout(
  "leaves an active pane undefined in an empty split",
  [{ id: "g1", root: split("horizontal", [], []), activeTabId: "missing" }],
  []
)
addSerializeLayout(
  "prints non-finite sizes as null and keeps the shortest form",
  [
    {
      id: "g1",
      root: row([pane("a"), pane("b"), pane("c")], [NaN, Infinity, 0.3333333333333333]),
      activeTabId: "b",
    },
  ],
  ["a", "b", "c"]
)

output.serializeTabGroups = serializedGroups

const regexAudit = []
const audit = (pattern, flags, positive, negative) => {
  const regex = new RegExp(pattern, flags)
  regexAudit.push({
    pattern,
    flags,
    positive: { input: positive, matches: new RegExp(pattern, flags).test(positive) },
    negative: { input: negative, matches: new RegExp(pattern, flags).test(negative) },
  })
}
audit("\\\\", "g", "a\\b", "ab")
audit("^/([A-Za-z])(/|$)", "", "/c/Users", "/1/Users")
audit("/+$", "", "C:/Users///", "C:/Users")
audit("^[A-Za-z]:", "", "C:/Users", "1:/Users")
audit("[\\\\/]+$", "", "C:\\Users\\\\", "C:\\Users")
audit("^[A-Za-z]:$", "", "C:", "C:\\")
audit("^(?:sudo|&|\\w+=\\S*)\\s+", "", "sudo /usr/bin/htop", "变量=值 cmd")
audit("^([\"'])(.*?)\\1", "", "\"C:\\Program Files\"", "\"unterminated'")
audit("\\s", "", "a b", "ab")
audit("\\s+", "g", "a  b", "ab")
audit("^.*[\\\\/]", "", "C:\\tools\\vim.exe", "vim.exe")
audit("\\.(exe|cmd|bat|com|ps1)$", "i", "BUILD.PS1", "build.ps1 ")
audit("[\\\\/]+", "", "a\\\\b/c", "abc")
audit("-\\d+$", "", "OS terminal-3", "Terminal-０")
audit("^-\\d+$", "", "-12", "-１２")
audit("^#[0-9a-f]{6}$", "i", "#AABBCC", "#GGGGGG")
output.regexAudit = regexAudit

process.stdout.write(`${JSON.stringify(output, null, 2)}\n`)
