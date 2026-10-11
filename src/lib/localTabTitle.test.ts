import { describe, expect, it } from "vitest"

import {
  commandTitle,
  directoryTitle,
  distinctLocalTitles,
  hasLegacyLocalTitle,
  runningCommandTitle,
  tabTooltip,
  withLocalCwd,
} from "@/lib/localTabTitle"
import type { Tab } from "@/types/tab"

describe("hasLegacyLocalTitle", () => {
  it("recognizes the old default names of local tabs only", () => {
    expect(hasLegacyLocalTitle({ type: "terminal", title: "OS terminal" })).toBe(true)
    expect(hasLegacyLocalTitle({ type: "terminal", title: "OS terminal-3" })).toBe(true)
    expect(hasLegacyLocalTitle({ type: "terminal", title: "本地终端" })).toBe(true)
    expect(hasLegacyLocalTitle({ type: "terminal", title: "builds" })).toBe(false)
    expect(hasLegacyLocalTitle({ type: "ssh", title: "Terminal" })).toBe(false)
  })
})

describe("directoryTitle", () => {
  it("names a directory by its last part", () => {
    expect(directoryTitle("D:\\code\\rust\\tTerm", null)).toBe("tTerm")
    expect(directoryTitle("/home/me/src/", null)).toBe("src")
    expect(directoryTitle("\\\\server\\share", null)).toBe("share")
  })

  it("shows the home directory as ~, however the shell spells it", () => {
    const home = "C:\\Users\\Me"
    expect(directoryTitle("C:\\Users\\Me", home)).toBe("~")
    expect(directoryTitle("c:/users/me/", home)).toBe("~")
    expect(directoryTitle("/c/Users/Me", home)).toBe("~")
    expect(directoryTitle("/home/me", "/home/me")).toBe("~")
    expect(directoryTitle("/home/me/src", "/home/me")).toBe("src")
  })

  it("keeps roots whole", () => {
    expect(directoryTitle("/", "/home/me")).toBe("/")
    expect(directoryTitle("C:\\", null)).toBe("C:\\")
    expect(directoryTitle("C:", null)).toBe("C:\\")
    expect(directoryTitle("  ", null)).toBeNull()
  })
})

describe("commandTitle", () => {
  it("shows a short command whole", () => {
    expect(commandTitle("npm run dev")).toBe("npm run dev")
    expect(commandTitle("  ssh prod  ")).toBe("ssh prod")
  })

  it("shows the program alone for a long command", () => {
    expect(commandTitle("cargo test --all-targets -- -D warnings")).toBe("cargo")
  })

  it("drops the program's path, extension and what runs it", () => {
    expect(commandTitle("C:\\tools\\vim.exe notes.txt")).toBe("vim notes.txt")
    expect(commandTitle('& "C:\\Program Files\\node.exe" app.js')).toBe("node app.js")
    expect(commandTitle("& .\\build.ps1")).toBe("build")
    expect(commandTitle("RUST_LOG=debug sudo /usr/bin/htop")).toBe("htop")
  })
})

describe("withLocalCwd", () => {
  const tab = (fields: Partial<Tab>): Tab => ({
    id: "tab-1",
    title: "pwsh",
    type: "terminal",
    isActive: true,
    ...fields,
  })

  it("names a tab named automatically after its directory", () => {
    const next = withLocalCwd(tab({ autoTitle: true }), "/work/tTerm")
    expect(next).toMatchObject({ title: "tTerm", connection: { cwd: "/work/tTerm" } })
  })

  it("keeps the name the user gave", () => {
    const next = withLocalCwd(tab({ title: "builds" }), "/work/tTerm")
    expect(next).toMatchObject({ title: "builds", connection: { cwd: "/work/tTerm" } })
  })

  it("returns the same tab when nothing changes", () => {
    const current = tab({ title: "tTerm", autoTitle: true, connection: { cwd: "/work/tTerm" } })
    expect(withLocalCwd(current, "/work/tTerm")).toBe(current)
  })
})

describe("tabTooltip", () => {
  it("adds a local tab's directory or a remote tab's host", () => {
    expect(
      tabTooltip({
        id: "1",
        title: "builds",
        type: "terminal",
        isActive: true,
        connection: { cwd: "/work" },
      })
    ).toBe("builds (/work)")
    expect(
      tabTooltip({ id: "1", title: "pwsh", type: "terminal", isActive: true, connection: {} })
    ).toBe("pwsh")
    expect(
      tabTooltip({
        id: "1",
        title: "prod",
        type: "ssh",
        isActive: true,
        connection: { host: "10.0.0.1" },
      })
    ).toBe("prod (10.0.0.1)")
  })
})

describe("distinctLocalTitles", () => {
  const local = (id: string, cwd: string, fields: Partial<Tab> = {}): Tab => ({
    id,
    title: "",
    type: "terminal",
    isActive: false,
    autoTitle: true,
    connection: { type: "terminal", cwd },
    ...fields,
  })
  const titles = (tabs: Tab[], home: string | null = null) =>
    distinctLocalTitles(tabs, home).map((tab) => tab.title)

  it("adds parent directories until same-named directories differ", () => {
    expect(
      titles([
        local("1", "/work/web/src"),
        local("2", "/work/api/src"),
        local("3", "/home/me/api/src"),
      ])
    ).toEqual(["web/src", "work/api/src", "me/api/src"])
  })

  it("keeps the plain name for tabs in the very same directory", () => {
    const tabs = [local("1", "D:\\code\\tTerm"), local("2", "d:/code/tTerm/")]
    expect(titles(tabs)).toEqual(["tTerm", "tTerm"])
  })

  it("separates Windows paths with backslashes", () => {
    expect(titles([local("1", "C:\\a\\src"), local("2", "C:\\b\\src")])).toEqual([
      "a\\src",
      "b\\src",
    ])
  })

  it("falls back to the whole path when one directory ends the other", () => {
    expect(titles([local("1", "/src"), local("2", "/a/src")])).toEqual(["/src", "a/src"])
  })

  it("leaves tabs the user named, and returns the same array when nothing changes", () => {
    const tabs = [
      local("1", "/a/src", { title: "src" }),
      local("2", "/b/src", { autoTitle: undefined, title: "mine" }),
    ]
    expect(titles(tabs)).toEqual(["src", "mine"])
    expect(distinctLocalTitles(tabs, null)).toBe(tabs)
  })

  it("shows the home directory as ~", () => {
    expect(titles([local("1", "/home/me")], "/home/me")).toEqual(["~"])
  })
})

describe("tabTooltip with a shell", () => {
  it("names the shell after the directory", () => {
    const tab: Tab = {
      id: "1",
      title: "tTerm",
      type: "terminal",
      isActive: true,
      shellName: "pwsh",
      connection: { cwd: "D:\\code\\tTerm" },
    }
    expect(tabTooltip(tab)).toBe("tTerm (D:\\code\\tTerm · pwsh)")
  })
})

describe("runningCommandTitle", () => {
  const local = (id: string, title: string, fields: Partial<Tab> = {}): Tab => ({
    id,
    title,
    type: "terminal",
    isActive: false,
    autoTitle: true,
    ...fields,
  })
  const commands = (running: Record<string, string>) => new Map(Object.entries(running))

  it("shows a running command alone while no other tab runs the same", () => {
    const running = commands({ "1": "claude", "2": "codex --yolo" })
    expect(runningCommandTitle(local("1", "tTerm"), running)).toBe("claude")
    expect(runningCommandTitle(local("2", "web"), running)).toBe("codex --yolo")
    expect(runningCommandTitle(local("3", "api"), running)).toBeNull()
  })

  it("adds the tab's title while another tab runs a command of the same title", () => {
    const running = commands({
      "1": "claude",
      "2": "claude --dangerously-skip-permissions",
      "3": "codex",
    })
    expect(runningCommandTitle(local("1", "tTerm"), running)).toBe("claude · tTerm")
    expect(runningCommandTitle(local("2", "web/src"), running)).toBe("claude · web/src")
    expect(runningCommandTitle(local("3", "api"), running)).toBe("codex")
  })

  it("leaves a tab the user named to its name", () => {
    const named = local("1", "Agent", { autoTitle: undefined })
    expect(runningCommandTitle(named, commands({ "1": "claude" }))).toBeNull()
  })
})
