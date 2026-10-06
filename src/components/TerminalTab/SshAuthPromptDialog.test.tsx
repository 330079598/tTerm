// @vitest-environment jsdom

import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react"
import { invoke } from "@tauri-apps/api/core"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import {
  SshAuthPromptDialog,
  type SshAuthPrompt,
} from "@/components/TerminalTab/SshAuthPromptDialog"

const listeners = new Map<string, (event: { payload: unknown }) => void>()

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, handler: (event: { payload: unknown }) => void) => {
    listeners.set(name, handler)
    return () => listeners.delete(name)
  }),
}))
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => undefined) }))
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))

const mockedInvoke = vi.mocked(invoke)

const otpPrompt: SshAuthPrompt = {
  requestId: "request-1",
  host: "bastion.example.com",
  port: 22,
  username: "ops",
  kind: "keyboard-interactive",
  name: "MFA",
  instructions: "Open your authenticator",
  prompts: [
    { text: "Username: ", echo: true },
    { text: "Verification code: ", echo: false },
  ],
}

async function renderDialog() {
  render(<SshAuthPromptDialog tabId="tab-1" />)
  await waitFor(() => expect(listeners.has("ssh-auth-prompt-expired-tab-1")).toBe(true))
}

function emit(name: string, payload: unknown) {
  act(() => listeners.get(name)!({ payload }))
}

beforeEach(() => {
  listeners.clear()
  mockedInvoke.mockClear()
})
afterEach(cleanup)

describe("SshAuthPromptDialog", () => {
  it("collects one answer per prompt and sends them to the backend", async () => {
    await renderDialog()
    expect(screen.queryByRole("dialog")).toBeNull()

    emit("ssh-auth-prompt-tab-1", otpPrompt)
    expect(screen.getByText("ops@bastion.example.com:22")).toBeTruthy()
    expect(screen.getByText("Open your authenticator")).toBeTruthy()

    const username = screen.getByLabelText("Username:")
    const code = screen.getByLabelText("Verification code:")
    expect(username.getAttribute("type")).toBe("text")
    expect(code.getAttribute("type")).toBe("password")

    fireEvent.change(username, { target: { value: "alice" } })
    fireEvent.change(code, { target: { value: "123456" } })
    fireEvent.submit(screen.getByRole("dialog").querySelector("form")!)

    await waitFor(() =>
      expect(mockedInvoke).toHaveBeenCalledWith("respond_ssh_auth_prompt", {
        requestId: "request-1",
        responses: ["alice", "123456"],
      })
    )
    expect(screen.queryByRole("dialog")).toBeNull()
  })

  it("shows queued prompts one after another and cancels with null", async () => {
    await renderDialog()
    emit("ssh-auth-prompt-tab-1", otpPrompt)
    emit("ssh-auth-prompt-tab-1", {
      ...otpPrompt,
      requestId: "request-2",
      kind: "password",
      name: "",
      instructions: "",
      hopIndex: 1,
      totalHops: 2,
      prompts: [{ text: "Password:", echo: false }],
    })

    fireEvent.click(screen.getByRole("button", { name: "common.cancel" }))
    await waitFor(() =>
      expect(mockedInvoke).toHaveBeenCalledWith("respond_ssh_auth_prompt", {
        requestId: "request-1",
        responses: null,
      })
    )

    // The password prompt of the jump host is next, labelled by tTerm.
    expect(screen.getByText("ssh.authPrompt.jumpHost")).toBeTruthy()
    expect(screen.getByLabelText("ssh.authPrompt.password")).toBeTruthy()
  })

  it("closes a prompt the backend stopped waiting for", async () => {
    await renderDialog()
    emit("ssh-auth-prompt-tab-1", otpPrompt)
    expect(screen.getByRole("dialog")).toBeTruthy()

    emit("ssh-auth-prompt-expired-tab-1", "request-1")
    expect(screen.queryByRole("dialog")).toBeNull()
    expect(mockedInvoke).not.toHaveBeenCalled()
  })
})
