import { beforeEach, expect, it, vi } from "vitest"
import { listenForHostKeyPromptExpiry } from "@/components/TerminalTab/hostKeyPromptExpiry"
import type { HostKeyPromptState } from "@/components/TerminalTab/types"

const listeners = new Map<string, (event: { payload: string }) => void>()
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, handler: (event: { payload: string }) => void) => {
    listeners.set(name, handler)
    return () => listeners.delete(name)
  }),
}))
beforeEach(() => listeners.clear())

it("dismisses only the matching expired host question and cleans up the listener", async () => {
  let current = { requestId: "new" } as HostKeyPromptState | null
  const off = await listenForHostKeyPromptExpiry("tab", (update) => {
    current = typeof update === "function" ? update(current) : update
  })
  const emit = listeners.get("ssh-hostkey-prompt-expired-tab")!
  emit({ payload: "old" })
  expect(current?.requestId).toBe("new")
  emit({ payload: "new" })
  expect(current).toBeNull()
  emit({ payload: "new" })
  expect(current).toBeNull()
  off()
  expect(listeners.size).toBe(0)
})
