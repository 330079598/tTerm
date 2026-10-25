import { describe, expect, it } from "vitest"

import {
  DEFAULT_TAB_ICON_CONFIG,
  normalizeSavedSshTab,
  normalizeTabIconConfig,
  normalizeTabIconStyle,
  resolveTabIcon,
  shellsForSettings,
  shellTabIcon,
  withLocalDistro,
  withShellIconStyle,
  type TabIconConfig,
} from "@/lib/tabIcons"
import type { Tab } from "@/types/tab"

const config = (fields: Partial<TabIconConfig> = {}): TabIconConfig => ({
  ...DEFAULT_TAB_ICON_CONFIG,
  ...fields,
})

describe("resolveTabIcon", () => {
  it("draws a shell with its built-in look until the user picks otherwise", () => {
    expect(resolveTabIcon({ type: "terminal", shellName: "pwsh" }, config())).toEqual({
      icon: "square-chevron-right",
      color: "#3b9ae1",
    })
    expect(resolveTabIcon({ type: "terminal", shellName: "nu" }, config())).toEqual({
      icon: "terminal",
      color: "",
    })
  })

  it("lays the shell's choice over the built-in look, and the tab's over both", () => {
    const icons = config({ shells: { pwsh: { color: "#e5534b" } } })
    expect(resolveTabIcon({ type: "terminal", shellName: "PWSH" }, icons)).toEqual({
      icon: "square-chevron-right",
      color: "#e5534b",
    })
    const tab = {
      type: "terminal" as const,
      shellName: "pwsh",
      iconStyle: { icon: "rocket" as const, color: "" },
    }
    expect(resolveTabIcon(tab, icons)).toEqual({ icon: "rocket", color: "" })
  })

  it("draws an SSH tab as a server until its host's distribution is known", () => {
    expect(resolveTabIcon({ type: "ssh" }, config())).toEqual({ icon: "server", color: "" })
    expect(resolveTabIcon({ type: "ssh", distroId: "debian" }, config())).toEqual({
      icon: "distro:debian",
      color: "",
    })
  })

  it("lays an SSH tab's own choice over its host's distribution", () => {
    const tab = {
      type: "ssh" as const,
      distroId: "ubuntu" as const,
      iconStyle: { icon: "database" as const, color: "#e5534b" },
    }
    expect(resolveTabIcon(tab, config())).toEqual({ icon: "database", color: "#e5534b" })
  })

  it("draws a WSL tab with the distribution its launcher reported", () => {
    expect(resolveTabIcon({ type: "terminal", shellName: "WSL" }, config())).toEqual({
      icon: "terminal",
      color: "#e9853a",
    })
    const tab = { type: "terminal" as const, shellName: "WSL", distroId: "arch" as const }
    expect(resolveTabIcon(tab, config())).toEqual({ icon: "distro:arch", color: "" })
  })

  it("lays the user's choice for WSL over the reported distribution", () => {
    const tab = { type: "terminal" as const, shellName: "WSL", distroId: "arch" as const }
    expect(resolveTabIcon(tab, config({ shells: { wsl: { icon: "rocket" } } }))).toEqual({
      icon: "rocket",
      color: "",
    })
  })

  it("keeps a distribution off a local tab that no longer runs WSL", () => {
    const tab = { type: "terminal" as const, shellName: "pwsh", distroId: "arch" as const }
    expect(resolveTabIcon(tab, config())?.icon).toBe("square-chevron-right")
  })

  it("shows nothing while icons are off, or for other tabs", () => {
    expect(resolveTabIcon({ type: "terminal" }, config({ enabled: false }))).toBeNull()
    expect(resolveTabIcon({ type: "ssh" }, config({ enabled: false }))).toBeNull()
    expect(resolveTabIcon({ type: "remote-file-editor" }, config())).toBeNull()
  })
})

describe("normalizeTabIconStyle", () => {
  it("keeps known icons and well-formed colors only", () => {
    expect(normalizeTabIconStyle({ icon: "fish", color: "#AABBCC" })).toEqual({
      icon: "fish",
      color: "#aabbcc",
    })
    expect(normalizeTabIconStyle({ icon: "unicorn", color: "red" })).toBeUndefined()
    expect(normalizeTabIconStyle({ color: "" })).toEqual({ color: "" })
    expect(normalizeTabIconStyle("pwsh")).toBeUndefined()
  })

  it("keeps the logos of known distributions only", () => {
    expect(normalizeTabIconStyle({ icon: "distro:arch" })).toEqual({ icon: "distro:arch" })
    expect(normalizeTabIconStyle({ icon: "distro:windows" })).toBeUndefined()
  })
})

describe("withLocalDistro", () => {
  const local = { id: "a", title: "WSL", type: "terminal", isActive: true } as Tab

  it("gives a local tab the reported distribution, once", () => {
    const next = withLocalDistro(local, "debian")
    expect(next.distroId).toBe("debian")
    expect(withLocalDistro(next, "debian")).toBe(next)
  })

  it("leaves an SSH tab's distribution to its host", () => {
    const ssh = { ...local, type: "ssh" } as Tab
    expect(withLocalDistro(ssh, "debian")).toBe(ssh)
  })
})

describe("normalizeSavedSshTab", () => {
  it("drops an unknown distribution and icon from a saved SSH tab", () => {
    const saved = {
      id: "a",
      title: "host",
      type: "ssh" as const,
      isActive: false,
      distroId: "beos",
      iconStyle: { icon: "unicorn" },
    } as unknown as Tab
    const tab = normalizeSavedSshTab(saved)
    expect(tab.distroId).toBeUndefined()
    expect(tab.iconStyle).toBeUndefined()
  })
})

describe("normalizeTabIconConfig", () => {
  it("shows icons unless turned off, and keys shells in lower case", () => {
    expect(normalizeTabIconConfig(undefined)).toEqual({ enabled: true, shells: {} })
    expect(
      normalizeTabIconConfig({
        enabled: false,
        shells: { "Git Bash": { icon: "box" }, cmd: { icon: "nope" } },
      })
    ).toEqual({ enabled: false, shells: { "git bash": { icon: "box" } } })
  })
})

describe("withShellIconStyle", () => {
  it("sets a shell's choice, and forgets it for none", () => {
    const set = withShellIconStyle(config(), "zsh", { icon: "star" })
    expect(set.shells).toEqual({ zsh: { icon: "star" } })
    expect(withShellIconStyle(set, "zsh", undefined).shells).toEqual({})
  })
})

describe("shellsForSettings", () => {
  it("lists the platform's shells, then others with a choice saved", () => {
    const icons = config({ shells: { nu: { icon: "box" }, zsh: { icon: "star" } } })
    expect(shellsForSettings("macos", icons)).toEqual(["zsh", "bash", "fish", "sh", "nu"])
    expect(shellsForSettings("windows", config())).toContain("git bash")
  })

  it("adds the shells found on this machine after the platform's", () => {
    expect(shellsForSettings("linux", config(), ["Nu", null, "zsh"])).toEqual([
      "zsh",
      "bash",
      "fish",
      "sh",
      "nu",
    ])
    const icons = config({ shells: { nu: { icon: "box" }, elvish: { icon: "star" } } })
    expect(shellsForSettings("windows", icons, ["nu"]).slice(-2)).toEqual(["nu", "elvish"])
  })
})

describe("shellTabIcon", () => {
  it("is what a shell's tabs show without a choice of their own", () => {
    expect(shellTabIcon(config(), "Git Bash")).toEqual({ icon: "git-branch", color: "#f05133" })
  })
})
