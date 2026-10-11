import { describe, expect, it } from "vitest"

import {
  commandTitle,
  directoryTitle,
  hasLegacyLocalTitle,
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
