import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import { invoke } from "@tauri-apps/api/core"

import { startAgentResume, type AgentResumeNotice } from "@/components/TerminalTab/agentResume"
import type { PromptEvent, ShellPromptState } from "@/components/TerminalTab/commandMarks"

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))

function setup(resume = true, savedCwd?: string) {
  let data: (value: string) => void = () => {}
  let key: () => void = () => {}
  let prompt: (event: PromptEvent) => void = () => {}
  let state: ShellPromptState = "unmarked"
  let allowed = true
  let notice: AgentResumeNotice | null = null
  const dispose = vi.fn()
  const controller = startAgentResume({
    term: {
      onData: (listener) => {
        data = listener
        return { dispose }
      },
      onKey: (listener) => {
        key = () => listener({ key: "a", domEvent: {} as KeyboardEvent })
        return { dispose }
      },
    },
    commandMarks: {
      onPrompt: (listener) => {
        prompt = listener
        return dispose
      },
      promptState: () => state,
    },
    resume: resume ? { agent: "codex", sessionId: "abc-123", cwd: savedCwd } : undefined,
    cwd: "/work dir",
    tabId: "test-tab",
    sessionNonce: 7,
    canSend: () => allowed,
    onNotice: (value) => {
      notice = value
    },
  })
  return {
    controller,
    dispose,
    data: (value: string) => data(value),
    key: () => key(),
    prompt: () => {
      state = "prompt"
      prompt({ kind: "prompt" })
    },
    busy: () => {
      state = "busy"
    },
    block: () => {
      allowed = false
    },
    notice: () => notice,
  }
}

const writes = () => vi.mocked(invoke).mock.calls.filter(([command]) => command === "write_pty")
const settle = async () => {
  await Promise.resolve()
  await Promise.resolve()
  await Promise.resolve()
}

beforeEach(() => {
  vi.useFakeTimers()
  vi.mocked(invoke)
    .mockReset()
    .mockImplementation(async (command) =>
      command === "build_agent_resume_command" ? "RUST_BUILT_COMMAND" : undefined
    )
})
afterEach(() => vi.useRealTimers())

describe("agent resume timing and manual fallback", () => {
  it("waits through SSH authentication and MOTD until ready and an OSC prompt", async () => {
    const test = setup()
    test.prompt()
    expect(invoke).not.toHaveBeenCalled()
    test.busy()
    test.controller.ready("zsh", true)
    expect(invoke).not.toHaveBeenCalled()
    test.prompt()
    await settle()
    expect(invoke).toHaveBeenCalledWith("build_agent_resume_command", {
      agent: "codex",
      sessionId: "abc-123",
      cwd: "/work dir",
      shell: "zsh",
    })
    expect(writes()).toEqual([
      ["write_pty", { tabId: "test-tab", sessionNonce: 7, data: "RUST_BUILT_COMMAND\r" }],
    ])
    test.controller.dispose()
  })

  it("uses the agent start directory and the live SSH shell rather than a saved shell", async () => {
    const test = setup(true, "/remote/start dir")
    test.controller.ready("fish", true)
    test.prompt()
    await settle()
    expect(invoke).toHaveBeenCalledWith("build_agent_resume_command", {
      agent: "codex",
      sessionId: "abc-123",
      cwd: "/remote/start dir",
      shell: "fish",
    })
    test.controller.dispose()
  })

  it("after 15 seconds shows a button and never sends on a late prompt", async () => {
    const test = setup()
    test.controller.ready("bash", true)
    vi.advanceTimersByTime(15_000)
    expect(test.notice()).toMatchObject({ reason: "timeout", enabled: true })
    test.prompt()
    await settle()
    expect(writes()).toHaveLength(0)
    test.notice()?.resume()
    await settle()
    expect(writes()).toHaveLength(1)
    expect(test.notice()).toBeNull()
  })

  it.each(["cmd", "pwsh"])("without integration %s only sends when clicked", async (shell) => {
    const test = setup()
    test.controller.ready(shell, false)
    expect(test.notice()).toMatchObject({ reason: "unavailable", enabled: true })
    test.prompt()
    await settle()
    expect(invoke).not.toHaveBeenCalled()
    test.notice()?.resume()
    await settle()
    expect(writes()).toHaveLength(1)
  })

  it("unknown SSH shell disables manual resume and never guesses bash", async () => {
    const test = setup()
    test.controller.ready(undefined, true)
    expect(test.notice()).toMatchObject({ reason: "unknownShell", enabled: false })
    test.notice()?.resume()
    test.prompt()
    vi.advanceTimersByTime(60_000)
    await settle()
    expect(invoke).not.toHaveBeenCalled()
  })

  it.each(["typing", "key", "paste"])(
    "%s cancels the notice and a pending command",
    async (kind) => {
      const test = setup()
      let resolve: (value: string) => void = () => {}
      vi.mocked(invoke).mockReturnValueOnce(
        new Promise<string>((done) => {
          resolve = done
        })
      )
      test.controller.ready("fish", true)
      test.prompt()
      if (kind === "key") test.key()
      else test.data(kind === "paste" ? "\x1b[200~text" : "a")
      resolve("RUST_BUILT_COMMAND")
      await settle()
      vi.advanceTimersByTime(20_000)
      expect(test.notice()).toBeNull()
      expect(writes()).toHaveLength(0)
    }
  )

  it("typing after a notice also invalidates a previously captured button", async () => {
    const test = setup()
    test.controller.ready("sh", false)
    const click = test.notice()?.resume
    test.data("draft")
    click?.()
    await settle()
    expect(test.notice()).toBeNull()
    expect(invoke).not.toHaveBeenCalled()
  })

  it("terminal focus replies do not cancel waiting", async () => {
    const test = setup()
    test.controller.ready("bash", true)
    test.data("\x1b[1;1R")
    test.prompt()
    await settle()
    expect(writes()).toHaveLength(1)
  })

  it("rejects a null Rust command on both automatic and button paths", async () => {
    vi.mocked(invoke).mockResolvedValue(null)
    for (const integration of [true, false]) {
      const test = setup()
      test.controller.ready("bash", integration)
      if (integration) test.prompt()
      else test.notice()?.resume()
      await settle()
      expect(test.notice()).toMatchObject({ reason: "unsafe", enabled: false })
      expect(writes()).toHaveLength(0)
      test.controller.dispose()
    }
  })

  it.each(["disconnect", "dispose", "sensitive prompt", "busy"])(
    "cancels pending automatic input on %s",
    async (reason) => {
      const test = setup()
      let resolve: (value: string) => void = () => {}
      vi.mocked(invoke).mockReturnValueOnce(
        new Promise<string>((done) => {
          resolve = done
        })
      )
      test.controller.ready("bash", true)
      test.prompt()
      if (reason === "disconnect") test.controller.disconnect()
      if (reason === "dispose") test.controller.dispose()
      if (reason === "sensitive prompt") test.block()
      if (reason === "busy") test.busy()
      resolve("RUST_BUILT_COMMAND")
      await settle()
      expect(writes()).toHaveLength(0)
    }
  )

  it("does nothing without a saved resume field", async () => {
    const test = setup(false)
    test.controller.ready("bash", true)
    test.prompt()
    vi.advanceTimersByTime(60_000)
    await settle()
    expect(test.notice()).toBeNull()
    expect(invoke).not.toHaveBeenCalled()
  })
})
