import { describe, expect, it } from "vitest"

import {
  normalizeSftpStartDirectory,
  sftpFallbackPath,
  sftpStartPath,
} from "@/lib/sftpStartDirectory"

describe("normalizeSftpStartDirectory", () => {
  it("keeps the known choices and drops anything else", () => {
    expect(normalizeSftpStartDirectory("home")).toBe("home")
    expect(normalizeSftpStartDirectory("last")).toBe("last")
    expect(normalizeSftpStartDirectory("terminal")).toBe("terminal")
    expect(normalizeSftpStartDirectory("cwd")).toBeUndefined()
    expect(normalizeSftpStartDirectory(undefined)).toBeUndefined()
  })
})

describe("sftpStartPath", () => {
  it("opens at the terminal's directory when the drawer goes there", () => {
    expect(sftpStartPath("terminal", true, "/var/log")).toBe("/var/log")
    expect(sftpStartPath("terminal", false, "/var/log")).toBe("/var/log")
    expect(sftpStartPath("home", true, "/var/log")).toBe("/var/log")
  })

  it("opens at home or the last directory otherwise", () => {
    expect(sftpStartPath("home", false, "/var/log")).toBe(".")
    expect(sftpStartPath("last", false, "/var/log")).toBeNull()
  })

  it("falls back to home when the terminal reported no directory", () => {
    expect(sftpStartPath("terminal", true, null)).toBe(".")
    expect(sftpStartPath("last", true, null)).toBeNull()
  })
})

describe("sftpFallbackPath", () => {
  it("leaves the last directory to the backend's own fallbacks", () => {
    expect(sftpFallbackPath("last")).toBeNull()
    expect(sftpFallbackPath("home")).toBe(".")
    expect(sftpFallbackPath("terminal")).toBe(".")
  })
})
