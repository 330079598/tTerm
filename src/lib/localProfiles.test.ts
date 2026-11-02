import { describe, expect, it } from "vitest"

import {
  defaultLocalProfileName,
  draftLocalProfileFromTab,
  localProfileChoice,
  localTerminalSettings,
} from "@/lib/localProfiles"
import type { Tab } from "@/types/tab"

describe("localProfileChoice", () => {
  it("keeps only what the saved shell uses", () => {
    expect(
      localProfileChoice({ shell: "wsl", wslDistro: "Ubuntu", customPath: "/bin/fish" })
    ).toEqual({ shell: "wsl", wslDistro: "Ubuntu" })
    expect(
      localProfileChoice({ shell: "custom", customPath: "nu", customArgs: "-l", shellId: "n" })
    ).toEqual({ shell: "custom", customPath: "nu", customArgs: "-l", shellId: "n" })
  })

  it("starts Auto when nothing is saved", () => {
    expect(localProfileChoice(undefined)).toEqual({ shell: "auto" })
  })
})

describe("localTerminalSettings", () => {
  it("trims the shell and directory and leaves out what is empty", () => {
    expect(
      localTerminalSettings({ shell: "custom", customPath: " nu ", customArgs: " " }, " ~/web ")
    ).toEqual({ shell: "custom", customPath: "nu", startDirectory: "~/web" })
    expect(localTerminalSettings({ shell: "auto" }, "")).toEqual({ shell: "auto" })
  })
})

describe("defaultLocalProfileName", () => {
  it("names a profile after the folder it starts in", () => {
    expect(defaultLocalProfileName("/srv/web/", "zsh")).toBe("web")
    expect(defaultLocalProfileName("C:\\Users\\me\\api", "pwsh")).toBe("api")
  })

  it("falls back to the shell's name, then to Terminal", () => {
    expect(defaultLocalProfileName("~", "zsh")).toBe("zsh")
    expect(defaultLocalProfileName("C:\\", "pwsh")).toBe("pwsh")
    expect(defaultLocalProfileName("", "")).toBe("Terminal")
  })
})

describe("draftLocalProfileFromTab", () => {
  it("saves a local tab's shell, its current directory and its name", () => {
    const tab: Tab = {
      id: "t1",
      title: "api",
      type: "terminal",
      isActive: true,
      connection: {
        type: "terminal",
        terminalShell: "custom",
        terminalShellCustomPath: "/opt/homebrew/bin/fish",
        terminalShellId: "fish",
        cwd: "/Users/me/api",
      },
    }
    expect(draftLocalProfileFromTab(tab)).toMatchObject({
      name: "api",
      connection_type: "terminal",
      local_terminal: {
        shell: "custom",
        customPath: "/opt/homebrew/bin/fish",
        shellId: "fish",
        startDirectory: "/Users/me/api",
      },
    })
  })
})
