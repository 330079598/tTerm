import { describe, expect, it } from "vitest"
import { parseCwdReport } from "@/lib/terminalCwd"

describe("parseCwdReport", () => {
  it("reads OSC 7 paths sent by tTerm's POSIX shell integration", () => {
    expect(parseCwdReport(7, "file:///home/me/project")).toBe("/home/me/project")
    expect(parseCwdReport(7, "file:///c/Users/me")).toBe("/c/Users/me")
    expect(parseCwdReport(7, "file://localhost/tmp")).toBe("/tmp")
  })

  it("decodes escapes and keeps raw characters", () => {
    expect(parseCwdReport(7, "file:///home/me/a b%2520中")).toBe("/home/me/a b%20中")
    expect(parseCwdReport(7, "file:///home/me/caf%C3%A9")).toBe("/home/me/café")
    expect(parseCwdReport(7, "file:///home/me/100%")).toBe("/home/me/100%")
  })

  it("turns a file URL with a drive letter into a Windows path", () => {
    expect(parseCwdReport(7, "file:///C:/Users/me")).toBe("C:/Users/me")
    expect(parseCwdReport(7, "file:///D:")).toBe("D:")
  })

  it("ignores OSC 7 from other hosts and malformed payloads", () => {
    expect(parseCwdReport(7, "file://build-server/home/me")).toBeNull()
    expect(parseCwdReport(7, "/home/me")).toBeNull()
    expect(parseCwdReport(7, "file://")).toBeNull()
  })

  it("reads OSC 9;9 paths from cmd and PowerShell", () => {
    expect(parseCwdReport(9, '9;"C:\\Users\\me\\My Projects"')).toBe("C:\\Users\\me\\My Projects")
    expect(parseCwdReport(9, "9;C:\\Windows")).toBe("C:\\Windows")
    expect(parseCwdReport(9, "9;\\\\server\\share\\dir")).toBe("\\\\server\\share\\dir")
  })

  it("ignores other OSC 9 messages", () => {
    expect(parseCwdReport(9, "4;1;50")).toBeNull()
    expect(parseCwdReport(9, "Build finished")).toBeNull()
    expect(parseCwdReport(9, '9;""')).toBeNull()
    expect(parseCwdReport(133, "A")).toBeNull()
  })
})
