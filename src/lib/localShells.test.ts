import { describe, expect, it } from "vitest"

import type { LocalShell } from "@/lib/appConfig"
import {
  buildShellEntries,
  defaultShellChoice,
  defaultShellKey,
  defaultShellUpdates,
  failedShellFallback,
  localShellConnection,
  movedLocalShell,
  ownShellChoice,
  programName,
  removedLocalShell,
  savedLocalShell,
  savedLocalShellUpdates,
  startsAsSaved,
  startsShell,
  toggledHiddenShell,
  withCurrentLocalShell,
  withLocalShell,
  type ShellConfig,
} from "@/lib/localShells"
import type { TerminalShellProfile } from "@/lib/terminalShells"

const t = (_key: string, defaultValue: string) => defaultValue

const nu: LocalShell = { id: "nu", name: "nu", path: "C:\\nu\\nu.exe", args: "" }
const msys: LocalShell = {
  id: "msys",
  name: "MSYS2",
  path: "C:\\msys64\\usr\\bin\\bash.exe",
  args: "--login -i",
}

function shellConfig(overrides: Partial<ShellConfig> = {}): ShellConfig {
  return {
    terminal_shell: "auto",
    terminal_shell_custom_path: "",
    terminal_shell_custom_args: "",
    terminal_shell_id: "",
    local_shells: [],
    hidden_shells: [],
    ...overrides,
  }
}

const windowsShells: TerminalShellProfile[] = [
  { shell: "auto", label: "Auto (recommended)", source: "tTerm default" },
  { shell: "cmd", label: "Command Prompt (cmd)", source: "C:\\Windows\\System32\\cmd.exe" },
  { shell: "pwsh", label: "PowerShell 7 (pwsh)", source: "C:\\PowerShell\\7\\pwsh.exe" },
  { shell: "custom", label: "Custom executable", source: "manual path" },
]

const unixShells: TerminalShellProfile[] = [
  { shell: "auto", label: "Auto (recommended)", source: "tTerm default" },
  { shell: "custom", label: "zsh", source: "/bin/zsh" },
  { shell: "custom", label: "fish", source: "/opt/homebrew/bin/fish" },
  { shell: "custom", label: "Custom executable", source: "manual path" },
]

describe("buildShellEntries", () => {
  it("lists Auto, the shells found here, then the user's own", () => {
    const entries = buildShellEntries(
      windowsShells,
      shellConfig({ local_shells: [nu, msys], hidden_shells: ["cmd"] }),
      t
    )
    expect(entries.map((entry) => [entry.key, entry.kind, entry.hidden])).toEqual([
      ["auto", "auto", false],
      ["cmd", "detected", true],
      ["pwsh", "detected", false],
      ["custom:nu", "custom", false],
      ["custom:msys", "custom", false],
    ])
    expect(entries[4]).toMatchObject({
      label: "MSYS2",
      detail: "C:\\msys64\\usr\\bin\\bash.exe --login -i",
      choice: {
        shell: "custom",
        customPath: "C:\\msys64\\usr\\bin\\bash.exe",
        customArgs: "--login -i",
        shellId: "msys",
      },
    })
  })

  it("keys a shell found on PATH by its path and shows one the user added once", () => {
    const fish: LocalShell = { id: "f", name: "", path: "/opt/homebrew/bin/fish", args: "" }
    const entries = buildShellEntries(unixShells, shellConfig({ local_shells: [fish] }), t)
    expect(entries.map((entry) => [entry.key, entry.label])).toEqual([
      ["auto", "Auto (recommended)"],
      ["/bin/zsh", "zsh"],
      ["custom:f", "fish"],
    ])
  })
})

describe("the default shell", () => {
  it("is found by the id of the user's shell, or else by its kind or path", () => {
    expect(defaultShellKey(shellConfig())).toBe("auto")
    expect(defaultShellKey(shellConfig({ terminal_shell: "pwsh" }))).toBe("pwsh")
    expect(
      defaultShellKey(
        shellConfig({ terminal_shell: "custom", terminal_shell_custom_path: "/bin/zsh" })
      )
    ).toBe("/bin/zsh")
    const listed = shellConfig({
      terminal_shell: "custom",
      terminal_shell_custom_path: "old",
      terminal_shell_id: "nu",
      local_shells: [nu],
    })
    expect(defaultShellKey(listed)).toBe("custom:nu")
  })

  it("starts with the current path of the user's shell", () => {
    const config = shellConfig({
      terminal_shell: "custom",
      terminal_shell_custom_path: "stale.exe",
      terminal_shell_id: "msys",
      local_shells: [msys],
    })
    expect(defaultShellChoice(config)).toEqual({
      shell: "custom",
      customPath: msys.path,
      customArgs: "--login -i",
      shellId: "msys",
    })
    expect(defaultShellChoice(shellConfig({ terminal_shell: "cmd" }))).toEqual({
      shell: "cmd",
      customPath: undefined,
      customArgs: undefined,
    })
  })

  it("is set with the path older versions read", () => {
    expect(
      defaultShellUpdates({ shell: "custom", customPath: " nu ", customArgs: "-l", shellId: "nu" })
    ).toEqual({
      terminal_shell: "custom",
      terminal_shell_custom_path: "nu",
      terminal_shell_custom_args: "-l",
      terminal_shell_id: "nu",
    })
    expect(defaultShellUpdates({ shell: "pwsh", customPath: "x" })).toEqual({
      terminal_shell: "pwsh",
      terminal_shell_custom_path: "",
      terminal_shell_custom_args: "",
      terminal_shell_id: "",
    })
  })
})

describe("the user's shells", () => {
  it("keep the default's path in step when the default is edited", () => {
    const config = shellConfig({
      terminal_shell: "custom",
      terminal_shell_custom_path: nu.path,
      terminal_shell_id: "nu",
      local_shells: [nu],
    })
    const edited = { ...nu, path: "D:\\nu.exe", args: "-l" }
    expect(savedLocalShell(config, edited)).toEqual({
      local_shells: [edited],
      terminal_shell_custom_path: "D:\\nu.exe",
      terminal_shell_custom_args: "-l",
    })
    expect(savedLocalShell(shellConfig({ local_shells: [nu] }), msys)).toEqual({
      local_shells: [nu, msys],
    })
  })

  it("leave Auto as the default when the default is removed", () => {
    const config = shellConfig({
      terminal_shell: "custom",
      terminal_shell_id: "nu",
      local_shells: [nu, msys],
    })
    expect(removedLocalShell(config, "nu")).toEqual({
      local_shells: [msys],
      terminal_shell: "auto",
      terminal_shell_custom_path: "",
      terminal_shell_custom_args: "",
      terminal_shell_id: "",
    })
    expect(removedLocalShell(config, "msys")).toEqual({ local_shells: [nu] })
  })

  it("move one place at a time and stop at either end", () => {
    expect(movedLocalShell([nu, msys], "msys", -1)).toEqual([msys, nu])
    expect(movedLocalShell([nu, msys], "nu", -1)).toEqual([nu, msys])
    expect(movedLocalShell([nu, msys], "missing", 1)).toEqual([nu, msys])
  })

  it("hide and show detected shells by key", () => {
    expect(toggledHiddenShell(["cmd"], "pwsh")).toEqual(["cmd", "pwsh"])
    expect(toggledHiddenShell(["cmd", "pwsh"], "cmd")).toEqual(["pwsh"])
  })
})

describe("local tab connections", () => {
  it("carry the user's shell id so a restart follows its edits", () => {
    const connection = localShellConnection(
      { shell: "custom", customPath: nu.path, shellId: "nu" },
      "C:\\src"
    )
    expect(connection).toEqual({
      type: "terminal",
      cwd: "C:\\src",
      terminalShell: "custom",
      terminalShellCustomPath: nu.path,
      terminalShellCustomArgs: undefined,
      terminalShellId: "nu",
    })
    const moved = { ...nu, path: "D:\\nu.exe" }
    expect(withCurrentLocalShell(connection, [moved])?.terminalShellCustomPath).toBe("D:\\nu.exe")
    // A removed shell keeps the path the tab was saved with.
    expect(withCurrentLocalShell(connection, [])).toBe(connection)
  })

  it("leave out custom fields for a built-in shell", () => {
    expect(localShellConnection({ shell: "pwsh", customPath: "x", shellId: "y" })).toEqual({
      type: "terminal",
      cwd: undefined,
      terminalShell: "pwsh",
      terminalShellCustomPath: undefined,
      terminalShellCustomArgs: undefined,
      terminalShellId: undefined,
    })
  })
})

describe("programName", () => {
  it("names a shell after its program", () => {
    expect(programName('"C:\\Program Files\\nu\\nu.exe"')).toBe("nu")
    expect(programName("/opt/homebrew/bin/fish")).toBe("fish")
    expect(programName("pwsh")).toBe("pwsh")
    expect(programName(".hidden")).toBe(".hidden")
  })
})

describe("restarting a local tab with another shell", () => {
  it("tells whether a tab starts the same shell", () => {
    const own = { shell: "custom" as const, customPath: nu.path, shellId: "nu" }
    expect(startsShell(undefined, { shell: "auto" })).toBe(true)
    expect(startsShell({ type: "terminal", terminalShell: "pwsh" }, { shell: "cmd" })).toBe(false)
    expect(startsShell(localShellConnection(own), own)).toBe(true)
    expect(startsShell(localShellConnection(own), { ...own, shellId: "other" })).toBe(false)
    expect(
      startsShell(
        { type: "terminal", terminalShell: "custom", terminalShellCustomPath: "/bin/zsh " },
        { shell: "custom", customPath: "/bin/zsh" }
      )
    ).toBe(true)
  })

  it("keeps the tab's directory and starts a new session", () => {
    const tab = {
      connection: localShellConnection({ shell: "custom", customPath: "gone" }, "/src"),
      sessionNonce: 3,
    }
    expect(withLocalShell(tab, { shell: "auto" })).toEqual({
      connection: localShellConnection({ shell: "auto" }, "/src"),
      sessionNonce: 4,
    })
  })

  it("tells whether a tab starts the user's shell as it is saved now", () => {
    const tab = localShellConnection(ownShellChoice(msys))
    expect(startsAsSaved(tab, msys)).toBe(true)
    expect(startsAsSaved(tab, { ...msys, args: " --login -i " })).toBe(true)
    expect(startsAsSaved(tab, { ...msys, args: "--login" })).toBe(false)
    expect(startsAsSaved(tab, { ...msys, path: "D:\\msys64\\usr\\bin\\bash.exe" })).toBe(false)
  })

  it("offers the default shell, or Auto when the default is the one that failed", () => {
    const nuDefault = shellConfig({
      terminal_shell: "custom",
      terminal_shell_custom_path: nu.path,
      terminal_shell_id: "nu",
      local_shells: [nu],
    })
    const msysTab = localShellConnection(ownShellChoice(msys))
    expect(failedShellFallback(msysTab, nuDefault)).toEqual({
      choice: ownShellChoice(nu),
      replacesDefault: false,
    })
    expect(failedShellFallback(localShellConnection(ownShellChoice(nu)), nuDefault)).toEqual({
      choice: { shell: "auto" },
      replacesDefault: true,
    })
    expect(failedShellFallback(msysTab, shellConfig())).toEqual({
      choice: { shell: "auto", customPath: undefined, customArgs: undefined },
      replacesDefault: false,
    })
    expect(failedShellFallback(localShellConnection({ shell: "auto" }), shellConfig())).toBe(
      undefined
    )
  })

  it("saves a shell and makes it the default in one change", () => {
    expect(savedLocalShellUpdates(shellConfig(), nu, true)).toEqual({
      local_shells: [nu],
      terminal_shell: "custom",
      terminal_shell_custom_path: nu.path,
      terminal_shell_custom_args: "",
      terminal_shell_id: "nu",
    })
    expect(savedLocalShellUpdates(shellConfig(), nu, false)).toEqual({ local_shells: [nu] })
  })
})
