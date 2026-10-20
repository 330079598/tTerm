// @vitest-environment jsdom
import { renderHook } from "@testing-library/react"
import type { Terminal } from "@xterm/xterm"
import { beforeEach, describe, expect, it, vi } from "vitest"

import type { CommandMarks } from "@/components/TerminalTab/commandMarks"
import { useCdInTerminal } from "@/components/TerminalTab/useCdInTerminal"

const mocks = vi.hoisted(() => ({ invoke: vi.fn(() => Promise.resolve()) }))

vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }))

function setup(runningProgram: boolean) {
  const term = { focus: vi.fn() }
  const setShowSftpDrawer = vi.fn()
  const { result } = renderHook(() =>
    useCdInTerminal({
      tabId: "tab",
      sessionNonce: 3,
      termRef: { current: term as unknown as Terminal },
      commandMarksRef: {
        current: { isRunningProgram: () => runningProgram } as unknown as CommandMarks,
      },
      setShowSftpDrawer,
    })
  )
  return { cd: result.current, term, setShowSftpDrawer }
}

beforeEach(() => {
  mocks.invoke.mockClear()
})

describe("useCdInTerminal", () => {
  it("types cd at the prompt of this session and shows the terminal", () => {
    const { cd, term, setShowSftpDrawer } = setup(false)
    expect(cd("/srv/my app")).toBe(true)
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
    expect(cd("/srv")).toBe(false)
    expect(mocks.invoke).not.toHaveBeenCalled()
    expect(setShowSftpDrawer).not.toHaveBeenCalled()
  })
})
