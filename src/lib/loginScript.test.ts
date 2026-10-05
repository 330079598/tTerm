import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import {
  LOGIN_SCRIPT_FALLBACK_IDLE_MS,
  LOGIN_SCRIPT_PROMPT_IDLE_MS,
  LoginScriptRunner,
  looksLikeShellPrompt,
  parseLoginScript,
} from "@/lib/loginScript"

describe("parseLoginScript", () => {
  it("splits on any line ending and skips blank lines", () => {
    expect(parseLoginScript("cd /srv/app\r\n\n  \rsudo -i  \n")).toEqual(["cd /srv/app", "sudo -i"])
  })

  it("keeps leading indentation", () => {
    expect(parseLoginScript("  echo hi")).toEqual(["  echo hi"])
  })

  it("treats a missing script as empty", () => {
    expect(parseLoginScript(undefined)).toEqual([])
    expect(parseLoginScript(null)).toEqual([])
  })
})

describe("looksLikeShellPrompt", () => {
  it("recognizes common prompts", () => {
    expect(looksLikeShellPrompt("user@host:~$ ")).toBe(true)
    expect(looksLikeShellPrompt("root@host:~# ")).toBe(true)
    expect(looksLikeShellPrompt("host% ")).toBe(true)
    expect(looksLikeShellPrompt("PS C:\\> ")).toBe(true)
    expect(looksLikeShellPrompt("~/src ❯ ")).toBe(true)
  })

  it("rejects password prompts and plain output", () => {
    expect(looksLikeShellPrompt("[sudo] password for user: ")).toBe(false)
    expect(looksLikeShellPrompt("Last login: Mon Oct  5 10:00:00 2026")).toBe(false)
    expect(looksLikeShellPrompt("")).toBe(false)
  })
})

describe("LoginScriptRunner", () => {
  let cursorLine: string
  let atPasswordPrompt: boolean
  let sent: string[]

  const createRunner = (lines: string[]) =>
    new LoginScriptRunner({
      lines,
      send: (line) => sent.push(line),
      readCursorLine: () => cursorLine,
      isAtPasswordPrompt: () => atPasswordPrompt,
    })

  beforeEach(() => {
    vi.useFakeTimers()
    cursorLine = ""
    atPasswordPrompt = false
    sent = []
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it("types one command per quiet prompt", () => {
    const runner = createRunner(["cd /srv", "ls"])
    cursorLine = "user@host:~$ "
    runner.noteOutput()
    vi.advanceTimersByTime(LOGIN_SCRIPT_PROMPT_IDLE_MS)
    expect(sent).toEqual(["cd /srv"])

    // Nothing more until the shell answers.
    vi.advanceTimersByTime(LOGIN_SCRIPT_FALLBACK_IDLE_MS * 2)
    expect(sent).toEqual(["cd /srv"])

    cursorLine = "user@host:/srv$ "
    runner.noteOutput()
    vi.advanceTimersByTime(LOGIN_SCRIPT_PROMPT_IDLE_MS)
    expect(sent).toEqual(["cd /srv", "ls"])
    expect(runner.done).toBe(true)
  })

  it("waits until output stops", () => {
    const runner = createRunner(["uptime"])
    cursorLine = "user@host:~$ "
    runner.noteOutput()
    vi.advanceTimersByTime(LOGIN_SCRIPT_PROMPT_IDLE_MS - 1)
    runner.noteOutput()
    vi.advanceTimersByTime(LOGIN_SCRIPT_PROMPT_IDLE_MS - 1)
    expect(sent).toEqual([])
    vi.advanceTimersByTime(1)
    expect(sent).toEqual(["uptime"])
  })

  it("falls back to a longer wait when the prompt is unusual", () => {
    createRunner(["id"])
    cursorLine = "[custom prompt] "
    vi.advanceTimersByTime(LOGIN_SCRIPT_PROMPT_IDLE_MS)
    expect(sent).toEqual([])
    vi.advanceTimersByTime(LOGIN_SCRIPT_FALLBACK_IDLE_MS)
    expect(sent).toEqual(["id"])
  })

  it("leaves a password prompt to the user", () => {
    const runner = createRunner(["sudo -i", "cd /root"])
    cursorLine = "user@host:~$ "
    runner.noteOutput()
    vi.advanceTimersByTime(LOGIN_SCRIPT_PROMPT_IDLE_MS)
    expect(sent).toEqual(["sudo -i"])

    cursorLine = "[sudo] password for user: "
    atPasswordPrompt = true
    runner.noteOutput()
    vi.advanceTimersByTime(LOGIN_SCRIPT_FALLBACK_IDLE_MS * 3)
    expect(sent).toEqual(["sudo -i"])

    cursorLine = "root@host:~# "
    atPasswordPrompt = false
    runner.noteOutput()
    vi.advanceTimersByTime(LOGIN_SCRIPT_PROMPT_IDLE_MS)
    expect(sent).toEqual(["sudo -i", "cd /root"])
  })

  it("sends nothing once stopped", () => {
    const runner = createRunner(["ls"])
    cursorLine = "user@host:~$ "
    runner.stop()
    runner.noteOutput()
    vi.advanceTimersByTime(LOGIN_SCRIPT_FALLBACK_IDLE_MS)
    expect(sent).toEqual([])
  })

  it("is done at once with no commands", () => {
    expect(createRunner([]).done).toBe(true)
  })
})
