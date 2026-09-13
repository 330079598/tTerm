// @vitest-environment jsdom

import { act, cleanup, renderHook, waitFor } from "@testing-library/react"
import { afterEach, describe, expect, it, vi } from "vitest"

import { useSftpUploads } from "@/components/SftpDrawer/useSftpUploads"
import type { TransferTask } from "@/types/tab"

type Listener = (event: { payload: unknown }) => void

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))
const { listeners } = vi.hoisted(() => ({ listeners: new Map<string, Listener>() }))

vi.mock("@tauri-apps/api/core", () => ({ invoke }))
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    listen: async (event: string, handler: Listener) => {
      listeners.set(event, handler)
      return () => listeners.delete(event)
    },
  }),
}))
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }))
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))

const batchStartEvent = "sftp-upload-batch-start-tab-1"
const batchCompleteEvent = "sftp-upload-batch-complete-tab-1"
const itemStartEvent = "sftp-upload-item-start-tab-1"
const itemProgressEvent = "sftp-upload-progress-tab-1"
const itemCompleteEvent = "sftp-upload-item-complete-tab-1"

function emit(event: string, payload: unknown) {
  const listener = listeners.get(event)
  if (!listener) throw new Error(`No listener registered for ${event}`)
  act(() => listener({ payload }))
}

function renderUploadsHook() {
  const transfers: TransferTask[] = []
  const addTransfer = vi.fn(
    (transfer: Omit<TransferTask, "id" | "startTime" | "status" | "transferred">, id?: string) => {
      const nextId = id ?? `generated-${transfers.length}`
      transfers.push({
        ...transfer,
        id: nextId,
        // A transfer started ten seconds ago keeps completion speeds
        // distinguishable from a zero-duration divide-by-zero guard.
        startTime: Date.now() - 10_000,
        status: "pending",
        transferred: 0,
      })
      return nextId
    }
  )
  const updateTransfer = vi.fn((id: string, updates: Partial<TransferTask>) => {
    const index = transfers.findIndex((item) => item.id === id)
    if (index >= 0) transfers[index] = { ...transfers[index], ...updates }
  })

  renderHook(() =>
    useSftpUploads({
      addTransfer,
      connection: undefined,
      lastProgressUpdateRef: { current: new Map<string, number>() },
      listing: { currentPath: "/remote", entries: [] },
      loadDirectory: vi.fn().mockResolvedValue(undefined),
      setError: vi.fn(),
      tabId: "tab-1",
      transfersRef: { current: transfers },
      updateTransfer,
    })
  )

  return { transfers }
}

describe("useSftpUploads", () => {
  afterEach(() => {
    cleanup()
    listeners.clear()
    vi.clearAllMocks()
  })

  it("retries a finished batch with the paths it was started with", async () => {
    const { transfers } = renderUploadsHook()
    await waitFor(() => expect(listeners.has(batchStartEvent)).toBe(true))

    emit(batchStartEvent, {
      batchId: "batch-1",
      displayName: "folder",
      localPath: "/local/folder",
      localPaths: ["/local/folder"],
      remoteBasePath: "/remote",
    })
    // The batch is over (failed) before the user can click retry: the retry
    // closure must still know which paths the batch was started with.
    emit(batchCompleteEvent, {
      batchId: "batch-1",
      cancelled: false,
      error: "boom",
      failed: 1,
      succeeded: 0,
    })

    const batch = transfers.find((item) => item.id === "batch-1")
    expect(batch?.status).toBe("failed")
    expect(batch?.retry).toBeTypeOf("function")

    invoke.mockResolvedValue({ cancelled: false, failed: 0, succeeded: 1 })
    act(() => batch?.retry?.())

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith(
        "sftp_upload_paths",
        expect.objectContaining({
          localPaths: ["/local/folder"],
          remoteBasePath: "/remote",
          skipExisting: true,
          tabId: "tab-1",
        })
      )
    )
  })

  it("excludes resumed bytes from the completion speed", async () => {
    const { transfers } = renderUploadsHook()
    await waitFor(() => expect(listeners.has(itemStartEvent)).toBe(true))

    emit(itemStartEvent, {
      batchId: undefined,
      fileName: "big.bin",
      fileSize: 1000,
      localPath: "/local/big.bin",
      remotePath: "/remote/big.bin",
      transferId: "item-1",
    })
    emit(itemProgressEvent, {
      progress: 100,
      resumedFrom: 900,
      transferred: 1000,
      total: 1000,
      transferId: "item-1",
    })
    emit(itemCompleteEvent, {
      cancelled: false,
      success: true,
      transferId: "item-1",
    })

    const transfer = transfers.find((item) => item.id === "item-1")
    // 100 resumed-free bytes over a ten second attempt ≈ 10 B/s. Counting the
    // resumed 900 bytes again would report ≈ 100 B/s.
    expect(transfer?.speed).toBeGreaterThan(0)
    expect(transfer?.speed).toBeLessThan(20)
  })

  it("reports no speed for an item the retry skipped", async () => {
    const { transfers } = renderUploadsHook()
    await waitFor(() => expect(listeners.has(itemStartEvent)).toBe(true))

    emit(itemStartEvent, {
      batchId: undefined,
      fileName: "unchanged.bin",
      fileSize: 1000,
      localPath: "/local/unchanged.bin",
      remotePath: "/remote/unchanged.bin",
      transferId: "item-2",
    })
    // No progress events: the backend skipped the transfer because the remote
    // already held the bytes, so no bytes moved and no speed may be invented.
    emit(itemCompleteEvent, {
      cancelled: false,
      skipped: true,
      success: true,
      transferId: "item-2",
    })

    const transfer = transfers.find((item) => item.id === "item-2")
    expect(transfer?.status).toBe("completed")
    expect(transfer?.speed).toBe(0)
  })
})
