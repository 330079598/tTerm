// @vitest-environment jsdom
import { Terminal } from "@xterm/xterm"
import { describe, expect, it } from "vitest"

import {
  compilePromptPatterns,
  isValidPromptPattern,
  matchPasswordPrompt,
  readCursorLine,
} from "@/lib/sudoPrompt"

describe("matchPasswordPrompt", () => {
  it("matches the default English sudo prompt", () => {
    expect(matchPasswordPrompt("[sudo] password for admin: ", "admin")).toEqual({
      prompt: "[sudo] password for admin:",
      user: "admin",
    })
  })

  it("matches the quoted user name of newer sudo releases", () => {
    expect(matchPasswordPrompt("[sudo] password for 'admin': ", "admin")?.user).toBe("admin")
  })

  it("matches sudo-rs prompts, which do not name the user", () => {
    expect(matchPasswordPrompt("[sudo: authenticate] Password: ", "stone")).toEqual({
      prompt: "[sudo: authenticate] Password:",
      user: null,
    })
    expect(matchPasswordPrompt("[sudo: authenticate] 密码：", "stone")?.user).toBeNull()
  })

  it("ignores sudo-rs prompts for a PIN or one-time code", () => {
    expect(matchPasswordPrompt("[sudo: authenticate] PIN: ", "stone")).toBeNull()
    expect(matchPasswordPrompt("[sudo: authenticate] Verification code: ", "stone")).toBeNull()
  })

  it("matches zh_CN and zh_TW sudo prompts with a full-width colon", () => {
    expect(matchPasswordPrompt("[sudo] stone 的密码：", undefined)?.user).toBe("stone")
    expect(matchPasswordPrompt("[sudo] stone 的密碼：", undefined)?.user).toBe("stone")
  })

  it("matches other sudo locales when the login user appears in the prompt", () => {
    expect(matchPasswordPrompt("[sudo] Passwort für stone: ", "stone")).toEqual({
      prompt: "[sudo] Passwort für stone:",
      user: "stone",
    })
    expect(matchPasswordPrompt("[sudo] mot de passe de stone : ", "stone")?.user).toBe("stone")
  })

  it("does not guess the user of an unknown sudo locale", () => {
    expect(matchPasswordPrompt("[sudo] Passwort für root: ", "stone")).toBeNull()
    expect(matchPasswordPrompt("[sudo] Passwort für stoneage: ", "stone")).toBeNull()
  })

  it("matches doas prompts", () => {
    expect(matchPasswordPrompt("doas (ops@web-1) password: ", "ops")?.user).toBe("ops")
  })

  it("reports the requested account so other users can be refused", () => {
    expect(matchPasswordPrompt("[sudo] password for root: ", "stone")?.user).toBe("root")
  })

  it("ignores ssh logins, shell prompts, and log lines", () => {
    expect(matchPasswordPrompt("stone@db-2's password: ", "stone")).toBeNull()
    expect(matchPasswordPrompt("stone@host:~$ ", "stone")).toBeNull()
    expect(matchPasswordPrompt("my password for fun: 123", "stone")).toBeNull()
    expect(matchPasswordPrompt("Password:", "stone")).toBeNull()
  })

  it("applies custom patterns with an optional user group", () => {
    const patterns = compilePromptPatterns([
      "^Password for (?<user>\\S+) on \\S+:$",
      "^Password:$",
      "(",
    ])
    expect(patterns).toHaveLength(2)
    expect(matchPasswordPrompt("Password for ops on db:", "ops", patterns)?.user).toBe("ops")
    expect(matchPasswordPrompt("Password:", "ops", patterns)).toEqual({
      prompt: "Password:",
      user: null,
    })
  })

  it("requires every prompt to end with a colon, custom ones included", () => {
    const patterns = compilePromptPatterns(["^Enter PIN>$"])
    expect(matchPasswordPrompt("Enter PIN>", "ops", patterns)).toBeNull()
  })
})

describe("isValidPromptPattern", () => {
  it("rejects malformed regular expressions", () => {
    expect(isValidPromptPattern("^Password:$")).toBe(true)
    expect(isValidPromptPattern("(")).toBe(false)
  })
})

describe("readCursorLine", () => {
  const write = (term: Terminal, data: string) =>
    new Promise<void>((resolve) => term.write(data, resolve))

  it("reads the visible text before the cursor, ignoring colors and redraws", async () => {
    const term = new Terminal({ cols: 80, rows: 10, allowProposedApi: true })
    await write(term, "$ sudo ls\r\n\x1b[1m[sudo]\x1b[0m password for ")
    await write(term, "stone: \x1b7\x1b[10;1Hstatus\x1b8")
    expect(readCursorLine(term.buffer.active).trim()).toBe("[sudo] password for stone:")
    term.dispose()
  })

  it("joins a prompt that wrapped across rows", async () => {
    const term = new Terminal({ cols: 12, rows: 10, allowProposedApi: true })
    await write(term, "[sudo] password for stone: ")
    expect(readCursorLine(term.buffer.active).trim()).toBe("[sudo] password for stone:")
    term.dispose()
  })
})
