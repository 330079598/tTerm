import { describe, expect, it } from "vitest"

import { summarizeTransfers } from "@/components/TransferManager/transferSummary"
import type { TransferTask } from "@/types/tab"

let nextId = 0

function transfer(overrides: Partial<TransferTask>): TransferTask {
  nextId += 1
  return {
    id: `t${nextId}`,
    direction: "download",
    localPath: "/tmp/fw.bin",
    remotePath: "fw.bin",
    fileName: "fw.bin",
    fileSize: 1000,
    transferred: 0,
    status: "transferring",
    startTime: 0,
    ...overrides,
  }
}

describe("summarizeTransfers", () => {
  it("names the one direction the active transfers share", () => {
    const summary = summarizeTransfers([
      transfer({ direction: "upload" }),
      transfer({ direction: "upload", status: "pending" }),
      transfer({ direction: "download", status: "completed" }),
    ])

    expect(summary.kind).toBe("upload")
    expect(summary.activeCount).toBe(2)
  })

  it("reports mixed when active transfers go both ways", () => {
    const summary = summarizeTransfers([
      transfer({ direction: "upload" }),
      transfer({ direction: "download" }),
    ])

    expect(summary.kind).toBe("mixed")
  })

  it("falls back to the history when nothing is active", () => {
    const summary = summarizeTransfers([
      transfer({ direction: "delete", status: "completed" }),
      transfer({ direction: "delete", status: "failed" }),
    ])

    expect(summary.kind).toBe("delete")
    expect(summary.activeCount).toBe(0)
    expect(summary.failedCount).toBe(1)
    expect(summary.progress).toBeNull()
  })

  it("sums bytes over active byte transfers with a known size", () => {
    const summary = summarizeTransfers([
      transfer({ fileSize: 1000, transferred: 500 }),
      transfer({ fileSize: 3000, transferred: 500, status: "pending" }),
      transfer({ direction: "delete", fileSize: 10, transferred: 10 }),
      transfer({ sizeUnknown: true, fileSize: 0, transferred: 9000 }),
      transfer({ fileSize: 5000, transferred: 5000, status: "completed" }),
    ])

    expect(summary.progress).toBeCloseTo(0.25)
  })

  it("has no share when no active transfer has a known size", () => {
    const summary = summarizeTransfers([transfer({ sizeUnknown: true, fileSize: 0 })])

    expect(summary.progress).toBeNull()
  })
})
