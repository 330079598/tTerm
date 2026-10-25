// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import { Terminal } from "@xterm/xterm"
import type { TFunction } from "i18next"

const invoke = vi.fn<(command: string, args?: unknown) => Promise<unknown>>()
const quitHandlers: (() => void)[] = []

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args?: unknown) => invoke(command, args),
}))
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((_event: string, handler: () => void) => {
    quitHandlers.push(handler)
    return Promise.resolve(() => {
      quitHandlers.splice(quitHandlers.indexOf(handler), 1)
    })
  }),
}))

type Module = typeof import("@/components/TerminalTab/savedTerminalContent")

const t = ((_key: string, options?: { time?: string }) => `Restored ${options?.time}`) as TFunction
const terminals: Terminal[] = []

/** The module with fresh state, as each launch has it. */
async function load(): Promise<Module> {
  vi.resetModules()
  return import("@/components/TerminalTab/savedTerminalContent")
}

function terminal() {
  const term = new Terminal({ cols: 80, rows: 24, allowProposedApi: true })
  terminals.push(term)
  const write = (data: string) => new Promise<void>((resolve) => term.write(data, resolve))
  return { term, write }
}

function text(term: Terminal): string {
  const buffer = term.buffer.active
  const lines: string[] = []
  for (let y = 0; y <= buffer.baseY + buffer.cursorY; y++) {
    lines.push(buffer.getLine(y)?.translateToString(true) ?? "")
  }
  return lines.join("\n")
}

function calls(command: string) {
  return invoke.mock.calls.filter(([name]) => name === command).map(([, args]) => args)
}

function savedTabs() {
  return calls("save_scrollback_snapshots").map((args) =>
    (args as { snapshots: { tabId: string }[] }).snapshots.map((snapshot) => snapshot.tabId)
  )
}

beforeEach(() => {
  invoke.mockReset()
  invoke.mockResolvedValue(undefined)
  quitHandlers.length = 0
})

afterEach(() => {
  for (const term of terminals.splice(0)) term.dispose()
})

describe("savedTerminalContent", () => {
  it("shows a tab's saved content once, above its new session", async () => {
    const content = await load()
    content.setTerminalContentRestore(true)
    invoke.mockResolvedValueOnce([{ tabId: "tab-1", content: "$ ls\r\nfile", savedAt: 0 }])
    await content.loadSavedTerminalContent(["tab-1"])
    expect(calls("load_scrollback_snapshots")).toEqual([{ tabIds: ["tab-1"] }])

    const first = terminal()
    content.restoreTerminalContent("tab-1", first.term, t)
    await first.write("$ ")
    expect(text(first.term)).toMatch(/^\$ ls\nfile\n── Restored .+ ──\n\$ $/)

    const again = terminal()
    content.restoreTerminalContent("tab-1", again.term, t)
    await again.write("$ ")
    expect(text(again.term)).toBe("$ ")
  })

  it("saves only the terminals that changed", async () => {
    const content = await load()
    content.setTerminalContentRestore(true)
    const one = terminal()
    const two = terminal()
    content.restoreTerminalContent("tab-1", one.term, t)
    content.restoreTerminalContent("tab-2", two.term, t)
    await one.write("hello")
    await content.saveTerminalContent()
    await content.saveTerminalContent()
    expect(savedTabs()).toEqual([["tab-1"]])
  })

  it("saves on a timer while on", async () => {
    vi.useFakeTimers()
    try {
      const content = await load()
      content.setTerminalContentRestore(true)
      const one = terminal()
      content.restoreTerminalContent("tab-1", one.term, t)
      one.term.write("hello")
      await vi.advanceTimersByTimeAsync(content.SAVE_INTERVAL_MS)
      expect(savedTabs()).toEqual([["tab-1"]])
    } finally {
      vi.useRealTimers()
    }
  })

  it("saves a failed save again the next time", async () => {
    const content = await load()
    content.setTerminalContentRestore(true)
    const one = terminal()
    content.restoreTerminalContent("tab-1", one.term, t)
    await one.write("hello")
    vi.spyOn(console, "error").mockImplementation(() => {})
    invoke.mockRejectedValueOnce(new Error("disk full"))
    await content.saveTerminalContent()
    await content.saveTerminalContent()
    expect(savedTabs()).toEqual([["tab-1"], ["tab-1"]])
  })

  it("stops tracking a terminal once it is disposed", async () => {
    const content = await load()
    content.setTerminalContentRestore(true)
    const one = terminal()
    content.restoreTerminalContent("tab-1", one.term, t).dispose()
    await one.write("hello")
    await content.saveTerminalContent()
    expect(savedTabs()).toEqual([])
  })

  it("saves nothing and deletes what was saved while off", async () => {
    const content = await load()
    content.setTerminalContentRestore(false)
    const one = terminal()
    content.restoreTerminalContent("tab-1", one.term, t)
    await one.write("hello")
    await content.saveTerminalContent()
    expect(savedTabs()).toEqual([])
    expect(calls("clear_scrollback_snapshots")).toHaveLength(1)
    expect(calls("set_save_before_quit")).toEqual([])
  })

  it("asks the app to wait for a save on quit, and lets it go once saved", async () => {
    const content = await load()
    content.setTerminalContentRestore(true)
    expect(calls("set_save_before_quit")).toEqual([{ enabled: true }])
    const one = terminal()
    content.restoreTerminalContent("tab-1", one.term, t)
    await one.write("hello")
    await vi.waitFor(() => expect(quitHandlers).toHaveLength(1))

    quitHandlers[0]()
    await vi.waitFor(() => expect(calls("ready_to_quit")).toHaveLength(1))
    expect(savedTabs()).toEqual([["tab-1"]])
    const order = invoke.mock.calls.map(([name]) => name)
    expect(order.indexOf("save_scrollback_snapshots")).toBeLessThan(order.indexOf("ready_to_quit"))

    content.setTerminalContentRestore(false)
    expect(calls("set_save_before_quit")).toEqual([{ enabled: true }, { enabled: false }])
    expect(quitHandlers).toHaveLength(0)
  })

  it("saves the terminals of before when turned on", async () => {
    const content = await load()
    const one = terminal()
    content.restoreTerminalContent("tab-1", one.term, t)
    await one.write("hello")
    content.setTerminalContentRestore(true)
    await content.saveTerminalContent()
    expect(savedTabs()).toEqual([["tab-1"]])
  })

  it("stops saving after a session is imported", async () => {
    const content = await load()
    content.setTerminalContentRestore(true)
    invoke.mockResolvedValueOnce([{ tabId: "tab-1", content: "old", savedAt: 0 }])
    await content.loadSavedTerminalContent(["tab-1"])
    content.suspendTerminalContentSaving()

    const one = terminal()
    content.restoreTerminalContent("tab-1", one.term, t)
    await one.write("new")
    await content.saveTerminalContent()
    expect(text(one.term)).toBe("new")
    expect(savedTabs()).toEqual([])
    const quitSettings = calls("set_save_before_quit")
    expect(quitSettings[quitSettings.length - 1]).toEqual({ enabled: false })
  })
})
