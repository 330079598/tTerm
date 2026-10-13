// @vitest-environment jsdom

import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react"
import { invoke } from "@tauri-apps/api/core"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { ProxyCommandPrompt } from "@/components/ProxyCommandPrompt"

const listeners = new Map<string, (event: { payload: unknown }) => void>()

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, handler: (event: { payload: unknown }) => void) => {
    listeners.set(name, handler)
    return () => listeners.delete(name)
  }),
}))
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => []) }))
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))

const mockedInvoke = vi.mocked(invoke)

const prompt = {
  requestId: "hash-1",
  proxyName: "Cloudflare",
  command: "cloudflared access ssh --hostname %h",
}

async function renderPrompt() {
  render(<ProxyCommandPrompt />)
  await waitFor(() => expect(listeners.has("proxy-command-prompt-expired")).toBe(true))
}

function emit(name: string, payload: unknown) {
  act(() => listeners.get(name)!({ payload }))
}

beforeEach(() => {
  listeners.clear()
  mockedInvoke.mockClear()
})
afterEach(cleanup)

describe("ProxyCommandPrompt", () => {
  it("shows the command and sends the answer", async () => {
    await renderPrompt()
    expect(screen.queryByRole("dialog")).toBeNull()

    emit("proxy-command-prompt", prompt)
    expect(screen.getByText(prompt.command)).toBeTruthy()
    fireEvent.click(screen.getByText("proxies.commandPrompt.allow"))

    expect(mockedInvoke).toHaveBeenCalledWith("respond_proxy_command_prompt", {
      requestId: "hash-1",
      allow: true,
    })
    expect(screen.queryByRole("dialog")).toBeNull()
  })

  it("refuses and drops an expired question", async () => {
    await renderPrompt()
    emit("proxy-command-prompt", prompt)
    emit("proxy-command-prompt", { ...prompt, requestId: "hash-2", command: "nc %h %p" })
    // The same question asked again is shown once.
    emit("proxy-command-prompt", prompt)

    fireEvent.click(screen.getByText("proxies.commandPrompt.deny"))
    expect(mockedInvoke).toHaveBeenCalledWith("respond_proxy_command_prompt", {
      requestId: "hash-1",
      allow: false,
    })
    expect(screen.getByText("nc %h %p")).toBeTruthy()

    emit("proxy-command-prompt-expired", "hash-2")
    expect(screen.queryByRole("dialog")).toBeNull()
  })

  it("shows questions asked before it mounted", async () => {
    mockedInvoke.mockResolvedValueOnce([prompt])
    await renderPrompt()
    await waitFor(() => expect(screen.getByText(prompt.command)).toBeTruthy())
  })
})
