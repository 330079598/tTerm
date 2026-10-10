import { describe, expect, it } from "vitest"

import {
  captureTerminalInput,
  EMPTY_COMMAND_CAPTURE_STATE,
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

  it("captures a large bracketed paste in one pass", () => {
    const lines = Array.from({ length: 20_000 }, (_, index) => `echo line ${index}`)
    const pasted = captureTerminalInput(
      EMPTY_COMMAND_CAPTURE_STATE,
      `\x1b[200~${lines.join("\r")}\x1b[201~`
    )
    expect(captureTerminalInput(pasted.state, "\r").commands).toEqual([lines.join("\n")])
  })

  it("inserts pasted text at the cursor", () => {
    let result = captureTerminalInput(EMPTY_COMMAND_CAPTURE_STATE, "git  -s")
    result = captureTerminalInput(result.state, "\x1b[D\x1b[D\x1b[D")
    result = captureTerminalInput(result.state, "\x1b[200~status\x1b[201~")
    expect(captureTerminalInput(result.state, "\r").commands).toEqual(["git status -s"])
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
})
