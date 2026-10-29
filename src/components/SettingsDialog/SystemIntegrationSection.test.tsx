// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react"
import { invoke } from "@tauri-apps/api/core"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { SystemIntegrationSection } from "@/components/SettingsDialog/SystemIntegrationSection"

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))
vi.mock("@/lib/uiPlatform", () => ({ getDetectedPlatform: () => "macos" }))

const mockedInvoke = vi.mocked(invoke)

beforeEach(() => {
  mockedInvoke.mockReset()
})
afterEach(cleanup)

describe("SystemIntegrationSection", () => {
  it("keeps a row whose status could not be read, and reads it again", async () => {
    mockedInvoke.mockImplementation((command) =>
      command === "shell_command_status"
        ? Promise.reject(new Error("registry locked"))
        : Promise.resolve({ supported: true, installed: true })
    )
    render(<SystemIntegrationSection />)
    expect(await screen.findByText("systemIntegration.loadFailed")).toBeTruthy()
    expect(screen.getAllByRole("switch")).toHaveLength(1)

    mockedInvoke.mockResolvedValue({ supported: true, installed: false })
    fireEvent.click(screen.getByText("systemIntegration.retry"))
    await waitFor(() => expect(screen.getAllByRole("switch")).toHaveLength(2))
    expect(screen.queryByText("systemIntegration.loadFailed")).toBeNull()
  })

  it("hides what the system does not support", async () => {
    mockedInvoke.mockResolvedValue({ supported: false, installed: false })
    const { container } = render(<SystemIntegrationSection />)
    await waitFor(() => expect(container.textContent).toBe(""))
  })
})
