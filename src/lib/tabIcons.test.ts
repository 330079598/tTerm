import { describe, expect, it } from "vitest"

import {
  DEFAULT_TAB_ICON_CONFIG,
  normalizeTabIconConfig,
  normalizeTabIconStyle,
  resolveTabIcon,
  shellsForSettings,
  shellTabIcon,
  withShellIconStyle,
  type TabIconConfig,
} from "@/lib/tabIcons"

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

  it("shows nothing while icons are off, or for remote tabs", () => {
    expect(resolveTabIcon({ type: "terminal" }, config({ enabled: false }))).toBeNull()
    expect(resolveTabIcon({ type: "ssh" }, config())).toBeNull()
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
})

describe("shellTabIcon", () => {
  it("is what a shell's tabs show without a choice of their own", () => {
    expect(shellTabIcon(config(), "Git Bash")).toEqual({ icon: "git-branch", color: "#f05133" })
  })
})
