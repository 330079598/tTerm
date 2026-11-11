// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react"
import { expect, it } from "vitest"
import { useSessionScopedState } from "@/components/TerminalTab/useSessionScopedState"
import type { HostKeyPromptState } from "@/components/TerminalTab/types"

it("an old expiry updater cannot clear a newer session's host question", () => {
  const { result, rerender } = renderHook(useSessionScopedState, {
    initialProps: { sessionResetKey: "tab:1" },
  })
  const oldSetter = result.current.setHostKeyPrompt
  rerender({ sessionResetKey: "tab:2" })
  act(() => result.current.setHostKeyPrompt({ requestId: "new" } as HostKeyPromptState))
  act(() => oldSetter(() => null))
  expect(result.current.hostKeyPrompt?.requestId).toBe("new")
  act(() => result.current.setHostKeyPrompt(() => null))
  expect(result.current.hostKeyPrompt).toBeNull()
})
