// @vitest-environment jsdom

import { cleanup, render, screen } from "@testing-library/react"
import { afterEach, describe, expect, it, vi } from "vitest"

import { TransferProgress } from "@/components/TransferManager/TransferProgress"
import type { TransferTask } from "@/types/tab"

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))

function transfer(overrides: Partial<TransferTask>): TransferTask {
  return {
    id: "t1",
    direction: "download",
    localPath: "/tmp/fw.bin",
    remotePath: "fw.bin",
    fileName: "fw.bin",
    fileSize: 4096,
    transferred: 1024,
    status: "transferring",
    startTime: 0,
    ...overrides,
  }
}

describe("TransferProgress", () => {
  afterEach(cleanup)

  it("shows the share of a known size", () => {
    render(<TransferProgress transfer={transfer({})} />)
    expect(screen.getByText("1.0 KB / 4.0 KB")).toBeTruthy()
    expect(screen.getByText("25.0%")).toBeTruthy()
  })

  it("shows only what arrived when the size is unknown", () => {
    render(<TransferProgress transfer={transfer({ fileSize: 0, sizeUnknown: true })} />)
    expect(screen.getByText("1.0 KB")).toBeTruthy()
    expect(screen.getByText("transfer.sizeUnknown")).toBeTruthy()
    expect(screen.queryByText(/%$/)).toBeNull()
  })
})
