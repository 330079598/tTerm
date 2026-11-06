import manifest from "@manifest"
import { describe, expect, it } from "vitest"
import { arch, eol, exeExtension, family, platform, type, version } from "@/parity/tauri/os"

describe("parity OS", () => {
  it("answers from the manifest's platform block", () => {
    expect(platform()).toBe(manifest.platform.os)
    expect(version()).toBe(manifest.platform.version)
    expect(family()).toBe(manifest.platform.family)
    expect(type()).toBe(manifest.platform.type)
    expect(arch()).toBe(manifest.platform.arch)
    expect(exeExtension()).toBe(manifest.platform.exeExtension)
    expect(eol()).toBe(manifest.platform.eol)
  })

  it("defaults to macOS", () => {
    expect(platform()).toBe("macos")
    expect(family()).toBe("unix")
    expect(arch()).toBe("aarch64")
  })
})
