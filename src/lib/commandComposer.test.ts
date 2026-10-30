import { describe, expect, it } from "vitest"

import {
  caretOnFirstLine,
  caretOnLastLine,
  composerInput,
  getComposerTarget,
  getComposerGuarded,
  getComposerText,
  historySuggestion,
  insertIntoComposer,
  registerComposerTarget,
  setComposerText,
  type ComposerTarget,
} from "@/lib/commandComposer"

describe("composerInput", () => {
  it("sends one line followed by Enter", () => {
    expect(composerInput("ls -la", true)).toBe("ls -la\r")
    expect(composerInput("ls -la", false)).toBe("ls -la\r")
  })

  it("sends an empty text as a bare Enter", () => {
    expect(composerInput("", true)).toBe("\r")
  })

  it("drops trailing line breaks, which would run empty commands", () => {
    expect(composerInput("uptime\n\n", true)).toBe("uptime\r")
  })

  it("pastes several lines when the program asked for bracketed paste", () => {
    expect(composerInput("cd /tmp\r\nls", true)).toBe("\x1b[200~cd /tmp\rls\x1b[201~\r")
  })

  it("types several lines as lines otherwise", () => {
    expect(composerInput("cd /tmp\nls", false)).toBe("cd /tmp\rls\r")
  })

  it("keeps an end marker in the text from ending the paste early", () => {
    expect(composerInput("echo a\n\x1b[201~rm x", true)).toBe("\x1b[200~echo a\rrm x\x1b[201~\r")
  })
})

describe("caret lines", () => {
  it("tells whether the caret is on the first or last line", () => {
    const text = "one\ntwo"
    expect(caretOnFirstLine(text, 2)).toBe(true)
    expect(caretOnFirstLine(text, 5)).toBe(false)
    expect(caretOnLastLine(text, 2)).toBe(false)
    expect(caretOnLastLine(text, 5)).toBe(true)
  })
})

describe("registerComposerTarget", () => {
  const target = (): ComposerTarget => ({
    input: () => {},
    bracketedPasteMode: () => false,
    promptState: () => "prompt",
    onPrompt: () => () => {},
  })

  it("keeps a newer session's target when the older one unregisters", () => {
    const older = target()
    const newer = target()
    const unregisterOlder = registerComposerTarget("tab-1", older)
    registerComposerTarget("tab-1", newer)
    unregisterOlder()
    expect(getComposerTarget("tab-1")).toBe(newer)
  })
})

describe("historySuggestion", () => {
  const history = ["git status", "git stash pop", "ls -la"]

  it("offers the newest command that goes on from the text", () => {
    expect(historySuggestion("git st", history)).toBe("git status")
    expect(historySuggestion("git stash", history)).toBe("git stash pop")
  })

  it("offers nothing for an empty text, a whole command, or several lines", () => {
    expect(historySuggestion("", history)).toBeUndefined()
    expect(historySuggestion("ls -la", history)).toBeUndefined()
    expect(historySuggestion("cd /tmp\ngit", history)).toBeUndefined()
  })
})

describe("insertIntoComposer", () => {
  it("adds a command on a line of its own after the text there", () => {
    setComposerText("")
    insertIntoComposer("uptime")
    expect(getComposerText()).toBe("uptime")
    insertIntoComposer("df -h")
    expect(getComposerText()).toBe("uptime\ndf -h")
    setComposerText("")
  })

  it("keeps a command that asks for confirmation guarded until the bar is emptied", () => {
    setComposerText("")
    insertIntoComposer("uptime")
    insertIntoComposer("reboot", "Restart")
    expect(getComposerGuarded()).toEqual(["Restart"])
    setComposerText("uptime")
    expect(getComposerGuarded()).toEqual(["Restart"])
    setComposerText("")
    expect(getComposerGuarded()).toEqual([])
  })
})
