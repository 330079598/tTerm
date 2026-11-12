export function reducerCases(tabsState, home = null) {
  const pane = tabId => ({ type: "pane", tabId })
  const row = ids => ({ type: "split", orientation: "horizontal", children: ids.map(pane), sizes: ids.map(() => 1 / ids.length) })
  const shape = { type: "split", orientation: "horizontal", children: [{ type: "pane" }, { type: "pane" }], sizes: [0.5, 0.5] }
  const tab = (id, fields = {}) => ({ id, title: id, type: "terminal", isActive: false, ...fields })
  const group = (id, ids, activeTabId = ids[0]) => ({ id, root: ids.length === 1 ? pane(ids[0]) : row(ids), activeTabId })
  const initial = () => ({ tabs: [tab("a", { isActive: true }), tab("b"), tab("c")], activeTabId: "a", groups: [group("group-1", ["a", "b"]), group("group-2", ["c"])], groupCount: 2 })
  const empty = () => ({ tabs: [], activeTabId: null, groups: [], groupCount: 0 })
  const cases = []
  function add(name, action, state = initial()) {
    const input = structuredClone(state)
    const wireAction = structuredClone(action)
    if (action.type === "update") {
      const patch = action.patch
      action = { type: "update", id: action.id, updater: tab => ({ ...tab, ...patch }) }
    }
    const result = tabsState.tabsReducer(state, action)
    const wireState = { tabs: result.tabs, activeTabId: result.activeTabId, groups: result.groups.map(group => ({ id: group.id, root: group.root, activeTabId: group.activeTabId, ...(group.zoomedTabId === undefined ? {} : { zoomedTabId: group.zoomedTabId }) })), groupCount: result.groupCount }
    cases.push({ name, state: input, action: wireAction, result, home, resultBytes: JSON.stringify(wireState), tabBytes: JSON.stringify(result.tabs) })
  }
  add("restore saved split and orphan", { type: "restore", tabs: [tab("a"), tab("b"), tab("c")], activeTabId: "b", layout: { version: 2, groups: [{ root: row(["a", "b"]), activeTabId: "a" }] } }, empty())
  add("restore empty session", { type: "restore", tabs: [], activeTabId: null }, initial())
  add("restore permissive session fields", { type: "restore", tabs: [{ id: "a", type: "ssh", isActive: "yes", sessionNonce: -1, connectionHeaderPinned: 0, connection: { port: "22", terminalShell: "nushell" }, future: true }], activeTabId: "a" }, empty())
  add("add new tab", { type: "add", tab: tab("d") })
  add("add to empty state", { type: "add", tab: tab("") }, empty())
  add("open new settings page", { type: "open-page", tab: tab("s", { type: "settings", title: "Settings" }) })
  add("open distinct unknown tab type", { type: "open-page", tab: tab("s", { type: "future-b" }) }, { ...initial(), tabs: [tab("a", { type: "future-a" }), tab("b"), tab("c")] })
  add("open same unknown tab type", { type: "open-page", tab: tab("s", { type: "future-a" }) }, { ...initial(), tabs: [tab("a", { type: "future-a" }), tab("b"), tab("c")] })
  const page = initial(); page.tabs.push(tab("s", { type: "settings", settingsRequest: 4294967295 })); page.groups.push(group("group-3", ["s"])); page.groupCount = 3
  add("open existing page and increment request without wrapping", { type: "open-page", tab: tab("unused", { type: "settings", settingsSection: "theme" }) }, page)
  add("open existing page with empty section", { type: "open-page", tab: tab("unused", { type: "settings", settingsSection: "" }) }, structuredClone(page))
  add("rename page", { type: "rename-page", tabType: "settings", title: "设置" }, structuredClone(page))
  add("rename missing page", { type: "rename-page", tabType: "logs", title: "Logs" })
  add("remove active pane chooses group fallback", { type: "remove", ids: ["a"] })
  add("remove unknown tab", { type: "remove", ids: ["missing"] })
  add("remove last tab", { type: "remove", ids: ["a"] }, { tabs: [tab("a", { isActive: true })], activeTabId: "a", groups: [group("group-1", ["a"])], groupCount: 1 })
  add("remove with explicit preferred focus", { type: "remove", ids: ["b"], options: { preferredActiveTabId: "c", activatePreferred: true } })
  add("set active", { type: "set-active", id: "b" })
  add("set active preserves unknown field key position", { type: "set-active", id: "a" }, { tabs: [{ id: "a", title: "t", type: "ssh", isActive: false, future: 1, hasConnected: false }], activeTabId: null, groups: [], groupCount: 0 })
  add("duplicate preserves existing key positions", { type: "duplicate", sourceId: "a", newId: "d" }, { ...initial(), tabs: [tab("a", { title: "t", future: 1, sessionNonce: 3, hasConnected: false, duplicateBaseTitle: "t" }), tab("b"), tab("c")] })
  add("set active preserves home title when loaded", { type: "set-active", id: "a" }, { tabs: [tab("a", { title: "~", autoTitle: true, connection: { cwd: "/home/me" } })], activeTabId: "a", groups: [group("group-1", ["a"])], groupCount: 1 })
  add("set missing active", { type: "set-active", id: "missing" })
  add("move group reorders tabs", { type: "move", fromIndex: 1, toIndex: 0 })
  add("move out of range", { type: "move", fromIndex: -1, toIndex: 9 })
  add("duplicate with title conflict and nonce wrap", { type: "duplicate", sourceId: "a", newId: "d", split: "right" }, { ...initial(), tabs: [tab("a", { title: "web", sessionNonce: 4294967295, isActive: true }), tab("b", { title: "web-2" }), tab("c", { title: "web-3" })] })
  add("duplicate automatic title keeps base", { type: "duplicate", sourceId: "a", newId: "d" }, { ...initial(), tabs: [tab("a", { title: "src", autoTitle: true, duplicateBaseTitle: "old", connection: { cwd: "/a/src" }, sessionNonce: -1 }), tab("b"), tab("c")] })
  add("duplicate missing tab", { type: "duplicate", sourceId: "missing", newId: "d" })
  add("duplicate page is ignored", { type: "duplicate", sourceId: "s", newId: "d" }, structuredClone(page))
  add("duplicate fractional nonce", { type: "duplicate", sourceId: "a", newId: "d" }, { ...initial(), tabs: [tab("a", { sessionNonce: 4294967295.75 }), tab("b"), tab("c")] })
  for (const sessionNonce of [" 5", "\uFEFF5", [5], [[5]], [], [null], [5, 6], true, false, null, {}, "0x5", "5 "]) {
    add(`duplicate coerces nonce ${JSON.stringify(sessionNonce)}`, { type: "duplicate", sourceId: "a", newId: "d" }, { ...initial(), tabs: [tab("a", { sessionNonce }), tab("b"), tab("c")] })
  }
  for (const autoTitle of [1, "yes", [], {}, 0, "", null]) {
    add(`duplicate uses JS truthiness ${JSON.stringify(autoTitle)}`, { type: "duplicate", sourceId: "a", newId: "d" }, { ...initial(), tabs: [tab("a", { autoTitle, title: "src", duplicateBaseTitle: "old", connection: { cwd: "/home/me" } }), tab("b"), tab("c")] })
  }
  add("duplicate null connection fills its type", { type: "duplicate", sourceId: "a", newId: "d" }, { ...initial(), tabs: [tab("a", { connection: null, sessionNonce: null }), tab("b"), tab("c")] })
  add("rename clears generated metadata", { type: "rename", id: "a", title: "manual" }, { ...initial(), tabs: [tab("a", { autoTitle: true, duplicateBaseTitle: "base" }), tab("b"), tab("c")] })
  add("rename missing tab", { type: "rename", id: "missing", title: "manual" })
  add("update tab and recompute active", { type: "update", id: "a", patch: { id: "updated", title: "updated", isActive: true } })
  add("update missing tab", { type: "update", id: "missing", patch: { title: "unused" } })
  add("place pane", { type: "place-pane", id: "c", targetId: "a", direction: "below" })
  add("place pane missing target still activates source", { type: "place-pane", id: "c", targetId: "missing", direction: "right" })
  add("merge group", { type: "merge-group", id: "c", targetId: "b", direction: "right" })
  add("merge same group", { type: "merge-group", id: "a", targetId: "b", direction: "right" })
  add("break out pane", { type: "break-out", id: "b", index: 0 })
  add("break out single pane", { type: "break-out", id: "c" })
  add("ungroup split", { type: "ungroup", id: "a" })
  add("ungroup missing pane", { type: "ungroup", id: "missing" })
  add("zoom pane", { type: "toggle-zoom", id: "b" })
  add("zoom single pane", { type: "toggle-zoom", id: "c" })
  add("resize split", { type: "resize-split", groupId: "group-1", path: [], sizes: [3, 1] })
  add("resize missing path", { type: "resize-split", groupId: "group-1", path: [99], sizes: [1] })
  add("apply template creates copies", { type: "apply-template", id: "c", root: shape, newIds: ["d"] })
  add("apply template to empty groups", { type: "apply-template", id: "a", root: shape, newIds: ["d"] }, { ...initial(), groups: [] })
  add("apply template lacks enough copies", { type: "apply-template", id: "c", root: shape, newIds: [] })
  add("apply smaller template moves leftovers", { type: "apply-template", id: "b", root: { type: "pane" }, newIds: [] })
  add("reopen group", { type: "reopen", tabs: [tab("d"), tab("e")], layout: { kind: "group", shape, activeIndex: 1, index: 1 } })
  add("reopen empty tabs", { type: "reopen", tabs: [], layout: { kind: "group", shape, activeIndex: 0, index: 0 } })
  add("reopen pane restores exact previous tree", { type: "reopen", tabs: [tab("d")], layout: { kind: "pane", closedId: "old", previousRoot: row(["a", "old", "b"]), anchor: { tabId: "a", direction: "right" } } })
  add("reopen pane inserts beside changed target", { type: "reopen", tabs: [tab("d")], layout: { kind: "pane", closedId: "old", previousRoot: row(["a", "old"]), anchor: { tabId: "b", direction: "below" } } })
  add("reopen pane without anchor gets own group", { type: "reopen", tabs: [tab("d")], layout: { kind: "pane", closedId: "old", previousRoot: pane("old"), anchor: null } })
  add("add triggers distinct local titles", { type: "add", tab: tab("d", { autoTitle: true, title: "src", connection: { cwd: "/b/src" } }) }, { ...initial(), tabs: [tab("a", { autoTitle: true, title: "src", connection: { cwd: "/a/src" } }), tab("b"), tab("c")] })
  return cases
}
