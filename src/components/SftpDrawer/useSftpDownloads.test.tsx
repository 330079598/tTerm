// @vitest-environment jsdom

import { act, cleanup, renderHook, waitFor } from "@testing-library/react"
import { afterEach, describe, expect, it, vi } from "vitest"

import { useSftpDownloads } from "@/components/SftpDrawer/useSftpDownloads"
import type { SftpDirectoryEntry } from "@/components/SftpDrawer/types"
import type { TransferTask } from "@/types/tab"

type Listener = (event: { payload: unknown }) => void

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))
const { listeners } = vi.hoisted(() => ({ listeners: new Map<string, Listener>() }))
const { openDialog } = vi.hoisted(() => ({ openDialog: vi.fn() }))

vi.mock("@tauri-apps/api/core", () => ({ invoke }))
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    listen: async (event: string, handler: Listener) => {
      listeners.set(event, handler)
      return () => listeners.delete(event)
    },
  }),
}))
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: openDialog, save: vi.fn() }))
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))

const itemStartEvent = "sftp-download-item-start-tab-1"
const itemProgressEvent = "sftp-download-progress-tab-1"
const itemCompleteEvent = "sftp-download-item-complete-tab-1"
const batchCompleteEvent = "sftp-download-batch-complete-tab-1"

const folder: SftpDirectoryEntry = {
  isDir: true,
  isSymlink: false,
  name: "folder",
  path: "/remote/folder",
  size: 0,
}

function emit(event: string, payload: unknown) {
  const listener = listeners.get(event)
  if (!listener) throw new Error(`No listener registered for ${event}`)
  act(() => listener({ payload }))
}

function createTransferStore() {
  const transfers: TransferTask[] = []
  const addTransfer = vi.fn(
    (transfer: Omit<TransferTask, "id" | "startTime" | "status" | "transferred">, id?: string) => {
      const nextId = id ?? `generated-${transfers.length}`
      transfers.push({
        ...transfer,
        id: nextId,
        startTime: Date.now(),
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

  return { addTransfer, transfers, updateTransfer }
}

/** Starts a folder download and returns the id of its (parent) transfer. */
async function startFolderDownload(
  downloadEntry: (entry: SftpDirectoryEntry) => Promise<void>,
  transfers: TransferTask[]
) {
  await waitFor(() => expect(listeners.has(batchCompleteEvent)).toBe(true))
  openDialog.mockResolvedValue("/local/target")
  // The download stays in flight: the batch completion event arrives while the
  // invoke is still pending, exactly as the backend emits it.
  invoke.mockReturnValue(new Promise(() => {}))
  await act(async () => {
    void downloadEntry(folder)
  })
  await waitFor(() => expect(transfers.some((item) => item.remotePath === folder.path)).toBe(true))
  return transfers.find((item) => item.remotePath === folder.path)!.id
}

describe("useSftpDownloads", () => {
  afterEach(() => {
    cleanup()
    listeners.clear()
    vi.clearAllMocks()
    vi.restoreAllMocks()
  })

  it("reports no speed for a folder retry whose files were all skipped", async () => {
    const { addTransfer, transfers, updateTransfer } = createTransferStore()
    const { result } = renderHook(() =>
      useSftpDownloads({
        addTransfer,
        connection: undefined,
        tabId: "tab-1",
        transfersRef: { current: transfers },
        updateTransfer,
      })
    )
    const nowSpy = vi.spyOn(Date, "now").mockReturnValue(1_000_000)
    const batchId = await startFolderDownload(result.current.downloadEntry, transfers)

    nowSpy.mockReturnValue(1_000_000 + 10_000)
    emit(batchCompleteEvent, {
      batchId,
      cancelled: false,
      skipped: 1000,
      total: 1000,
      transferred: 1000,
    })

    const batch = transfers.find((item) => item.id === batchId)
    // Every byte already existed locally: counting them would report 1000 B
    // over a ten-second retry as if it had been downloaded.
    expect(batch?.speed).toBe(0)
    expect(batch?.status).toBe("completed")
  })

  it("excludes skipped bytes from the batch speed of a partial retry", async () => {
    const { addTransfer, transfers, updateTransfer } = createTransferStore()
    const { result } = renderHook(() =>
      useSftpDownloads({
        addTransfer,
        connection: undefined,
        tabId: "tab-1",
        transfersRef: { current: transfers },
        updateTransfer,
      })
    )
    const nowSpy = vi.spyOn(Date, "now").mockReturnValue(1_000_000)
    const batchId = await startFolderDownload(result.current.downloadEntry, transfers)

    nowSpy.mockReturnValue(1_000_000 + 10_000)
    emit(batchCompleteEvent, {
      batchId,
      cancelled: false,
      skipped: 400,
      total: 1000,
      transferred: 1000,
    })

    const batch = transfers.find((item) => item.id === batchId)
    // 600 fetched bytes over ten seconds ≈ 60 B/s. Counting the skipped 400
    // would report ≈ 100 B/s.
    expect(batch?.speed).toBeGreaterThan(0)
    expect(batch?.speed).toBeLessThan(70)
  })

  it("excludes resumed bytes from the batch speed after a partial attempt", async () => {
    const { addTransfer, transfers, updateTransfer } = createTransferStore()
    const { result } = renderHook(() =>
      useSftpDownloads({
        addTransfer,
        connection: undefined,
        tabId: "tab-1",
        transfersRef: { current: transfers },
        updateTransfer,
      })
    )
    const nowSpy = vi.spyOn(Date, "now").mockReturnValue(1_000_000)
    const batchId = await startFolderDownload(result.current.downloadEntry, transfers)

    nowSpy.mockReturnValue(1_000_000 + 10_000)
    emit(batchCompleteEvent, {
      batchId,
      cancelled: false,
      resumed: 900,
      skipped: 0,
      total: 1000,
      transferred: 1000,
    })

    const batch = transfers.find((item) => item.id === batchId)
    // Only 100 bytes were fetched; the other 900 were already in the part
    // file from the previous attempt. Counting them would report ≈ 100 B/s.
    expect(batch?.speed).toBeGreaterThan(0)
    expect(batch?.speed).toBeLessThan(20)
  })

  it("excludes resumed bytes and skipped items from the item speed", async () => {
    const { addTransfer, transfers, updateTransfer } = createTransferStore()
    renderHook(() =>
      useSftpDownloads({
        addTransfer,
        connection: undefined,
        tabId: "tab-1",
        transfersRef: { current: transfers },
        updateTransfer,
      })
    )
    await waitFor(() => expect(listeners.has(itemStartEvent)).toBe(true))
    const nowSpy = vi.spyOn(Date, "now").mockReturnValue(2_000_000)

    emit(itemStartEvent, {
      batchId: "batch-1",
      fileName: "big.bin",
      fileSize: 1000,
      localPath: "/local/big.bin",
      remotePath: "/remote/big.bin",
      transferId: "item-1",
    })
    emit(itemProgressEvent, {
      progress: 100,
      resumedFrom: 900,
      total: 1000,
      transferId: "item-1",
      transferred: 1000,
    })
    nowSpy.mockReturnValue(2_000_000 + 10_000)
    emit(itemCompleteEvent, {
      cancelled: false,
      success: true,
      transferId: "item-1",
    })

    const resumed = transfers.find((item) => item.id === "item-1")
    // 100 resumed-free bytes over ten seconds ≈ 10 B/s. Counting the resumed
    // 900 bytes again would report ≈ 100 B/s.
    expect(resumed?.speed).toBeGreaterThan(0)
    expect(resumed?.speed).toBeLessThan(20)

    emit(itemStartEvent, {
      batchId: "batch-1",
      fileName: "unchanged.bin",
      fileSize: 1000,
      localPath: "/local/unchanged.bin",
      remotePath: "/remote/unchanged.bin",
      transferId: "item-2",
    })
    // No progress events: the retry skipped the file because the local copy
    // already held the remote bytes, so no speed may be invented.
    emit(itemCompleteEvent, {
      cancelled: false,
      skipped: true,
      success: true,
      transferId: "item-2",
    })

    const skipped = transfers.find((item) => item.id === "item-2")
    expect(skipped?.status).toBe("completed")
    expect(skipped?.speed).toBe(0)
  })
})
