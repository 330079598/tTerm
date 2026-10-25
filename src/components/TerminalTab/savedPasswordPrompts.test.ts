// @vitest-environment jsdom

import { act, renderHook } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"

import { createSavedPasswordPrompts } from "@/components/TerminalTab/savedPasswordPrompts"
import type { TerminalSessionContext } from "@/components/TerminalTab/terminalLifecycleTypes"
import type { SavedPasswordPromptState, SudoPasswordSource } from "@/components/TerminalTab/types"
import { useTabPasswordPrompt } from "@/lib/tabAttention"
import { EMPTY_COMMAND_CAPTURE_STATE } from "@/lib/terminalCommandCapture"

const screen = vi.hoisted(() => ({ line: "", source: null as SudoPasswordSource | null }))

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async () => screen.source),
}))

vi.mock("@/lib/sudoPrompt", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/lib/sudoPrompt")>()),
  readCursorLine: () => screen.line,
}))

const SUDO_PROMPT = "[sudo] password for alice:"

function setup() {
  const shown: (SavedPasswordPromptState | null)[] = []
  const ctx = {
    term: { buffer: { active: {} } },
    session: {
      disposed: false,
      passwordPromptCheckId: 0,
      commandCaptureState: EMPTY_COMMAND_CAPTURE_STATE,
      commandCaptureSuspended: false,
      lineFeedCount: 0,
      currentPromptKey: null,
      lastPasswordFill: null,
      loginScriptRunner: null,
    },
    tabId: "tab",
    sessionNonce: 1,
    connectionRef: {
      current: { type: "ssh", username: "alice", profileId: "p1", profileName: "web" },
    },
    onSensitivePromptRef: { current: vi.fn() },
    onSavedPasswordPromptChange: vi.fn(),
    setSavedPasswordPrompt: (state: SavedPasswordPromptState | null) => shown.push(state),
    sudoPromptPatternsRef: { current: [] },
    onInputRef: { current: undefined },
    translationRef: { current: (key: string) => key },
    savedPasswordPromptActionsRef: { current: null },
  } as unknown as TerminalSessionContext
  const prompts = createSavedPasswordPrompts(ctx)
  const showPrompt = async (line: string) => {
    screen.line = line
    prompts.checkPasswordPrompt()
    // Lets the saved-password lookup settle.
    await Promise.resolve()
    await Promise.resolve()
  }
  return { ctx, prompts, shown, showPrompt, last: () => shown[shown.length - 1] }
}

describe("saved password prompts", () => {
  beforeEach(() => {
    screen.line = ""
    screen.source = null
  })

  it("counts what the user types when no saved password answers the prompt", async () => {
    const { prompts, showPrompt, last } = setup()
    await showPrompt(SUDO_PROMPT)
    expect(last()).toEqual({ status: "manual", prompt: SUDO_PROMPT, user: "alice", typed: 0 })

    prompts.typeAtPrompt("s")
    prompts.typeAtPrompt("ecret")
    expect(last()).toMatchObject({ status: "manual", typed: 6 })
    prompts.typeAtPrompt("\x7f")
    expect(last()).toMatchObject({ typed: 5 })

    prompts.typeAtPrompt("\r")
    expect(last()).toBeNull()
  })

  it("turns an offer into the count once the user types the password", async () => {
    screen.source = "login"
    const { prompts, showPrompt, last } = setup()
    await showPrompt(SUDO_PROMPT)
    expect(last()?.status).toBe("available")

    prompts.typeAtPrompt("s")
    expect(last()).toMatchObject({ status: "manual", typed: 1 })
  })

  it("leaves the user to type the password once the offer is dismissed", async () => {
    screen.source = "login"
    const { ctx, prompts, showPrompt, last } = setup()
    await showPrompt(SUDO_PROMPT)
    ctx.savedPasswordPromptActionsRef.current?.dismiss()
    expect(last()).toMatchObject({ status: "manual", typed: 0 })

    prompts.typeAtPrompt("s")
    expect(last()).toMatchObject({ status: "manual", typed: 1 })
  })

  it("marks the tab while the prompt waits, until the answer is submitted", async () => {
    const { prompts, showPrompt } = setup()
    const { result } = renderHook(() => useTabPasswordPrompt(["tab"]))
    await act(() => showPrompt(SUDO_PROMPT))
    expect(result.current).toBe(true)

    act(() => prompts.typeAtPrompt("s"))
    expect(result.current).toBe(true)

    act(() => prompts.typeAtPrompt("\r"))
    expect(result.current).toBe(false)
  })

  it("shows nothing for typing at the shell", async () => {
    const { prompts, shown, showPrompt } = setup()
    await showPrompt("alice@web:~$ ")
    shown.length = 0

    prompts.typeAtPrompt("l")
    expect(shown.every((state) => state === null)).toBe(true)
  })
})
