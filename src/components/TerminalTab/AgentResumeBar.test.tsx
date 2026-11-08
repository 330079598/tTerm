// @vitest-environment jsdom
import { fireEvent, render, screen, cleanup } from "@testing-library/react"
import { afterEach, expect, it, vi } from "vitest"
import { AgentResumeBar } from "@/components/TerminalTab/AgentResumeBar"

vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (key: string) => key }) }))
afterEach(cleanup)

it("shows the manual button without sending until clicked", () => {
  const resume = vi.fn()
  render(<AgentResumeBar notice={{ reason: "timeout", enabled: true, resume }} />)
  expect(screen.getByRole("status").textContent).toContain("agentResume.timeout")
  expect(resume).not.toHaveBeenCalled()
  fireEvent.click(screen.getByRole("button", { name: "agentResume.continue" }))
  expect(resume).toHaveBeenCalledTimes(1)
})

it("explains unknown shell and disables the resume button", () => {
  const resume = vi.fn()
  render(<AgentResumeBar notice={{ reason: "unknownShell", enabled: false, resume }} />)
  const button = screen.getByRole("button") as HTMLButtonElement
  expect(button.disabled).toBe(true)
  expect(screen.getByRole("status").textContent).toContain("agentResume.unknownShell")
  fireEvent.click(button)
  expect(resume).not.toHaveBeenCalled()
})
