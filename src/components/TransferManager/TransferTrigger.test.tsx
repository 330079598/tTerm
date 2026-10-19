// @vitest-environment jsdom

import { cleanup, render, screen } from "@testing-library/react"
import { afterEach, describe, expect, it, vi } from "vitest"

import { TransferTrigger } from "@/components/TransferManager/TransferTrigger"
import type { TransferTask } from "@/types/tab"

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) =>
      options ? `${key} ${JSON.stringify(options)}` : key,
  }),
}))

function transfer(overrides: Partial<TransferTask>): TransferTask {
  return {
    id: "t1",
    direction: "upload",
    localPath: "/tmp/fw.bin",
    remotePath: "fw.bin",
    fileName: "fw.bin",
    fileSize: 1000,
    transferred: 400,
    status: "transferring",
    startTime: 0,
    ...overrides,
  }
}

function renderTrigger(transfers: TransferTask[]) {
  render(<TransferTrigger transfers={transfers} isOpen={false} onToggle={() => {}} />)
  return screen.getByRole("button")
}

describe("TransferTrigger", () => {
  afterEach(cleanup)

  it("names the active count and share moved, and counts only active transfers", () => {
    const button = renderTrigger([
      transfer({}),
      transfer({ id: "t2", status: "completed", transferred: 1000 }),
    ])

    expect(button.getAttribute("aria-label")).toBe(
      'transfer.trigger.activeWithProgress {"count":1,"percent":40}'
    )
    expect(button.textContent).toContain("1")
    expect(button.querySelector("svg circle")).not.toBeNull()
  })

  it("leaves the share out when no active transfer knows its size", () => {
    const button = renderTrigger([transfer({ sizeUnknown: true, fileSize: 0 })])

    expect(button.getAttribute("aria-label")).toBe('transfer.trigger.active {"count":1}')
  })

  it("counts failures once nothing is active", () => {
    const button = renderTrigger([
      transfer({ status: "failed" }),
      transfer({ id: "t2", status: "completed" }),
    ])

    expect(button.getAttribute("aria-label")).toBe('transfer.trigger.failed {"count":1}')
    expect(button.querySelector('[data-variant="destructive"]')?.textContent).toBe("1")
    expect(button.querySelector("svg circle")).toBeNull()
  })

  it("counts every finished transfer when none failed", () => {
    const button = renderTrigger([
      transfer({ status: "completed" }),
      transfer({ id: "t2", status: "cancelled" }),
    ])

    expect(button.getAttribute("aria-label")).toBe('transfer.trigger.idle {"count":2}')
    expect(button.querySelector('[data-variant="secondary"]')?.textContent).toBe("2")
  })
})
