// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react"
import { invoke } from "@tauri-apps/api/core"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { RemoteAgentIntegrationDialog } from "@/components/RemoteAgentIntegrationDialog"
import type { AgentIntegrationStatus } from "@/types/agentIntegration"

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))

const mockedInvoke = vi.mocked(invoke)
const connection = { type: "ssh" as const, host: "web01", username: "me" }

function claude(state: AgentIntegrationStatus["state"]): AgentIntegrationStatus {
  return {
    agent: "claudeCode",
    kind: "hooks",
    state,
    detected: true,
    configDir: "/home/me/.claude",
    configPath: "/home/me/.claude/settings.json",
    configSource: "default",
    preview: "{}",
    error: null,
  }
}

beforeEach(() => {
  mockedInvoke.mockReset()
})
afterEach(cleanup)

describe("RemoteAgentIntegrationDialog", () => {
  it("reads the server's agents and installs on the tab's host", async () => {
    mockedInvoke.mockResolvedValueOnce([claude("notInstalled")])
    render(<RemoteAgentIntegrationDialog tabId="tab-1" connection={connection} onClose={vi.fn()} />)
    expect(mockedInvoke).toHaveBeenCalledWith("remote_agent_integration_status", {
      tabId: "tab-1",
      connection,
    })

    mockedInvoke.mockResolvedValueOnce(claude("installed"))
    fireEvent.click(await screen.findByText("notifications.agents.install"))
    await waitFor(() => expect(screen.getByText("notifications.agents.uninstall")).toBeTruthy())
    expect(mockedInvoke).toHaveBeenLastCalledWith("change_remote_agent_integration", {
      tabId: "tab-1",
      connection,
      agent: "claudeCode",
      install: true,
    })
  })

  it("says why the server could not be read and tries again", async () => {
    mockedInvoke.mockRejectedValueOnce("sh: not found")
    render(<RemoteAgentIntegrationDialog tabId="tab-1" connection={connection} onClose={vi.fn()} />)
    expect(await screen.findByText("remoteAgents.loadFailed")).toBeTruthy()

    mockedInvoke.mockResolvedValueOnce([claude("installed")])
    fireEvent.click(screen.getByText("remoteAgents.retry"))
    expect(await screen.findByText("notifications.agents.uninstall")).toBeTruthy()
    expect(mockedInvoke).toHaveBeenCalledTimes(2)
  })
})
