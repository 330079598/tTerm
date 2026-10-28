// @vitest-environment jsdom
import { renderHook } from "@testing-library/react"
import type { Terminal } from "@xterm/xterm"
import { beforeEach, describe, expect, it, vi } from "vitest"

import type { CommandMarks } from "@/components/TerminalTab/commandMarks"
import { useCdInTerminal } from "@/components/TerminalTab/useCdInTerminal"
import { setTabRemoteAwayHost } from "@/lib/remoteCwd"

const mocks = vi.hoisted(() => ({ invoke: vi.fn(() => Promise.resolve()) }))

vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }))

function setup(runningProgram: boolean, awaitsPrompt = false) {
  const term = { focus: vi.fn() }
  const setShowSftpDrawer = vi.fn()
  const { result } = renderHook(() =>
    useCdInTerminal({
      tabId: "tab",
      sessionNonce: 3,
      termRef: { current: term as unknown as Terminal },
      commandMarksRef: {
        current: {
          isRunningProgram: () => runningProgram,
          awaitsPrompt: () => awaitsPrompt,
        } as unknown as CommandMarks,
      },
      setShowSftpDrawer,
    })
  )
  return { cd: result.current, term, setShowSftpDrawer }
}

beforeEach(() => {
  mocks.invoke.mockClear()
  setTabRemoteAwayHost("tab", null)
})

describe("useCdInTerminal", () => {
  it("types cd at the prompt of this session and shows the terminal", () => {
    const { cd, term, setShowSftpDrawer } = setup(false)
    expect(cd("/srv/my app")).toBeNull()
    expect(mocks.invoke).toHaveBeenCalledWith("write_pty", {
      tabId: "tab",
      sessionNonce: 3,
      data: "cd /srv/my\\ app\r",
    })
    expect(setShowSftpDrawer).toHaveBeenCalledWith(false)
    expect(term.focus).toHaveBeenCalled()
  })

  it("sends nothing while a program has the terminal", () => {
    const { cd, setShowSftpDrawer } = setup(true)
    expect(cd("/srv")).toEqual({ reason: "busy" })
    expect(mocks.invoke).not.toHaveBeenCalled()
    expect(setShowSftpDrawer).not.toHaveBeenCalled()
  })

  it("sends nothing while the shell has not come back from a line", () => {
    const { cd } = setup(false, true)
    expect(cd("/srv")).toEqual({ reason: "busy" })
    expect(mocks.invoke).not.toHaveBeenCalled()
  })

  it("sends nothing while the terminal is on another host", () => {
    setTabRemoteAwayHost("tab", "db01")
    const { cd } = setup(false)
    expect(cd("/srv")).toEqual({ reason: "elsewhere", host: "db01" })
    expect(mocks.invoke).not.toHaveBeenCalled()
  })
})
