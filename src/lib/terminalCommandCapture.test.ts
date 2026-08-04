import { describe, expect, it } from "vitest"

import {
  captureTerminalInput,
  EMPTY_COMMAND_CAPTURE_STATE,
  isSaveCommandShortcut,
  parseShellIntegrationCommand,
} from "@/lib/terminalCommandCapture"

describe("terminal command capture", () => {
  it("captures typed commands on enter", () => {
    const typed = captureTerminalInput(EMPTY_COMMAND_CAPTURE_STATE, "git status")
    expect(captureTerminalInput(typed.state, "\r").commands).toEqual(["git status"])
  })

  it("tracks cursor edits and backspace", () => {
    let result = captureTerminalInput(EMPTY_COMMAND_CAPTURE_STATE, "git sttus")
    result = captureTerminalInput(result.state, "\x1b[D\x1b[D\x1b[D")
    result = captureTerminalInput(result.state, "a")
    expect(captureTerminalInput(result.state, "\r").commands).toEqual(["git status"])
  })

  it("keeps bracketed multiline paste as one command", () => {
    const pasted = captureTerminalInput(
      EMPTY_COMMAND_CAPTURE_STATE,
      "\x1b[200~printf 'a'\nprintf 'b'\x1b[201~"
    )
    expect(captureTerminalInput(pasted.state, "\r").commands).toEqual(["printf 'a'\nprintf 'b'"])
  })

  it("does not invent commands after shell-history navigation", () => {
    const navigated = captureTerminalInput(EMPTY_COMMAND_CAPTURE_STATE, "\x1b[A\r")
    expect(navigated.commands).toEqual([])
  })

  it("reads command lines supplied by shell integration", () => {
    expect(parseShellIntegrationCommand("E;docker ps")).toBe("docker ps")
    expect(parseShellIntegrationCommand("E;printf a; printf b")).toBe("printf a; printf b")
    expect(parseShellIntegrationCommand("D;0")).toBeNull()
  })

  it("recognizes the cross-platform save-selection shortcut", () => {
    expect(isSaveCommandShortcut({ ctrlKey: true, metaKey: false, shiftKey: true, key: "S" })).toBe(
      true
    )
    expect(isSaveCommandShortcut({ ctrlKey: false, metaKey: true, shiftKey: true, key: "s" })).toBe(
      true
    )
    expect(
      isSaveCommandShortcut({ ctrlKey: true, metaKey: false, shiftKey: false, key: "s" })
    ).toBe(false)
  })
})
