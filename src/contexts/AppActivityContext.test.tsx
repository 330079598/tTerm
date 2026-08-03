// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react"
import { useEffect } from "react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { AppActivityProvider, useAppActivity } from "@/contexts/AppActivityContext"

const mocks = vi.hoisted(() => ({ relaunchApp: vi.fn() }))

vi.mock("@/lib/updater", () => ({ relaunchApp: mocks.relaunchApp }))
vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (_key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? _key,
  }),
}))

function RestartHarness() {
  const { requestAppRestart, setActiveTerminalSessionCount } = useAppActivity()

  useEffect(() => {
    setActiveTerminalSessionCount(2)
  }, [setActiveTerminalSessionCount])

  return <button onClick={() => void requestAppRestart()}>Request restart</button>
}

function DirectRestartHarness() {
  const { requestAppRestart } = useAppActivity()

  return <button onClick={() => void requestAppRestart()}>Request restart</button>
}

describe("AppActivityProvider", () => {
  beforeEach(() => {
    mocks.relaunchApp.mockReset().mockResolvedValue(undefined)
  })

  it("requires confirmation before restarting with active terminal sessions", async () => {
    render(
      <AppActivityProvider>
        <RestartHarness />
      </AppActivityProvider>
    )

    fireEvent.click(screen.getByRole("button", { name: "Request restart" }))
    expect(await screen.findByText("Restart and interrupt active sessions?")).not.toBeNull()

    fireEvent.click(screen.getByRole("button", { name: "Cancel" }))
    expect(mocks.relaunchApp).not.toHaveBeenCalled()

    fireEvent.click(screen.getByRole("button", { name: "Request restart" }))
    fireEvent.click(await screen.findByRole("button", { name: "Restart anyway" }))
    await waitFor(() => expect(mocks.relaunchApp).toHaveBeenCalledOnce())
  })

  it("restarts immediately when there are no active terminal sessions", async () => {
    render(
      <AppActivityProvider>
        <DirectRestartHarness />
      </AppActivityProvider>
    )

    fireEvent.click(screen.getByRole("button", { name: "Request restart" }))

    await waitFor(() => expect(mocks.relaunchApp).toHaveBeenCalledOnce())
    expect(screen.queryByText("Restart and interrupt active sessions?")).toBeNull()
  })
})

afterEach(cleanup)
