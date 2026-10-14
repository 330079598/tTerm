import { describe, expect, it } from "vitest"
import { dropQuotingFor, droppedPathsText, quoteDroppedPath } from "@/lib/droppedPaths"

describe("dropQuotingFor", () => {
  it("escapes POSIX style outside Windows, whatever the shell", () => {
    expect(dropQuotingFor("macos", "zsh")).toBe("posix")
    expect(dropQuotingFor("linux", "fish")).toBe("posix")
    expect(dropQuotingFor("linux", undefined)).toBe("posix")
  })

  it("follows the shell on Windows", () => {
    expect(dropQuotingFor("windows", "pwsh")).toBe("powershell")
    expect(dropQuotingFor("windows", "PowerShell")).toBe("powershell")
    expect(dropQuotingFor("windows", "cmd")).toBe("cmd")
    expect(dropQuotingFor("windows", "WSL")).toBe("wsl")
    expect(dropQuotingFor("windows", "Git Bash")).toBe("git-bash")
  })

  it("gives an unknown Windows shell double quotes, like Windows Terminal", () => {
    expect(dropQuotingFor("windows", "nu")).toBe("cmd")
    expect(dropQuotingFor("windows", undefined)).toBe("cmd")
  })
})

describe("quoteDroppedPath", () => {
  it("escapes POSIX specials with backslashes and leaves other text alone", () => {
    expect(quoteDroppedPath("/Users/me/notes.txt", "posix")).toBe("/Users/me/notes.txt")
    expect(quoteDroppedPath("/Users/me/My File (1).txt", "posix")).toBe(
      "/Users/me/My\\ File\\ \\(1\\).txt"
    )
    expect(quoteDroppedPath("/tmp/it's $HOME", "posix")).toBe("/tmp/it\\'s\\ \\$HOME")
    expect(quoteDroppedPath("/Users/me/文档/报告.pdf", "posix")).toBe("/Users/me/文档/报告.pdf")
  })

  it("quotes a line break instead of escaping it into a continuation", () => {
    expect(quoteDroppedPath("/tmp/a\nb", "posix")).toBe("/tmp/a'\n'b")
  })

  it("puts a PowerShell path in single quotes only when it needs them", () => {
    expect(quoteDroppedPath("C:\\Users\\me\\a.txt", "powershell")).toBe("C:\\Users\\me\\a.txt")
    expect(quoteDroppedPath("C:\\Program Files\\app", "powershell")).toBe(
      "'C:\\Program Files\\app'"
    )
    expect(quoteDroppedPath("C:\\tmp\\$it's", "powershell")).toBe("'C:\\tmp\\$it''s'")
  })

  it("puts a cmd path in double quotes only when it needs them", () => {
    expect(quoteDroppedPath("C:\\Users\\me\\a.txt", "cmd")).toBe("C:\\Users\\me\\a.txt")
    expect(quoteDroppedPath("C:\\Program Files\\app", "cmd")).toBe('"C:\\Program Files\\app"')
    expect(quoteDroppedPath("C:\\a&b", "cmd")).toBe('"C:\\a&b"')
  })

  it("turns a drive path into a mount path for WSL and Git Bash", () => {
    expect(quoteDroppedPath("C:\\Users\\me\\a b.txt", "wsl")).toBe("/mnt/c/Users/me/a\\ b.txt")
    expect(quoteDroppedPath("D:\\", "wsl")).toBe("/mnt/d")
    expect(quoteDroppedPath("C:\\Users\\me\\a b.txt", "git-bash")).toBe("/c/Users/me/a\\ b.txt")
  })

  it("turns a path inside a WSL distribution into the distribution's own path", () => {
    expect(quoteDroppedPath("\\\\wsl.localhost\\Ubuntu\\home\\me\\x", "wsl")).toBe("/home/me/x")
    expect(quoteDroppedPath("\\\\wsl$\\Ubuntu", "wsl")).toBe("/")
  })

  it("single-quotes any other network path for WSL", () => {
    expect(quoteDroppedPath("\\\\server\\share\\it's", "wsl")).toBe("'\\\\server\\share\\it'\\''s'")
  })
})

describe("droppedPathsText", () => {
  it("joins the paths with spaces and ends with one", () => {
    expect(droppedPathsText(["/a b", "/c"], "posix")).toBe("/a\\ b /c ")
  })

  it("types nothing for no paths", () => {
    expect(droppedPathsText([], "posix")).toBe("")
    expect(droppedPathsText([""], "cmd")).toBe("")
  })
})
