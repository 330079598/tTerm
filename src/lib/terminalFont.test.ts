import { describe, expect, it } from "vitest"

import { defaultTerminalFontFamily, firstInstalledFont } from "@/lib/terminalFont"

describe("defaultTerminalFontFamily", () => {
  it("puts a Nerd Font before each system's monospace font", () => {
    expect(defaultTerminalFontFamily("macos")).toMatch(/^"JetBrains Mono Nerd Font", .*Menlo/)
    expect(defaultTerminalFontFamily("windows")).toMatch(/^"JetBrains Mono Nerd Font", .*Consolas/)
    expect(defaultTerminalFontFamily("linux")).toMatch(/^"JetBrains Mono Nerd Font", .*DejaVu/)
  })

  it("gives an unknown platform Linux's fonts", () => {
    expect(defaultTerminalFontFamily("freebsd")).toBe(defaultTerminalFontFamily("linux"))
  })
})

describe("firstInstalledFont", () => {
  const chain = '"JetBrains Mono Nerd Font", "JetBrains Mono", Menlo, monospace'

  it("names the first font of the chain the system has", () => {
    expect(firstInstalledFont(chain, ["Menlo", "jetbrains mono"])).toBe("JetBrains Mono")
    expect(firstInstalledFont(chain, ["Menlo"])).toBe("Menlo")
  })

  it("keeps the chain when none of it is installed", () => {
    expect(firstInstalledFont(chain, [])).toBe(chain)
  })
})
