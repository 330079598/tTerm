import { isRemoteOperation } from "@/components/TransferManager/transferDisplay"
import type { TransferDirection, TransferTask } from "@/types/tab"

/** What the transfers being summed up move: one direction, or more than one. */
export type TransferKind = TransferDirection | "mixed"

export interface TransferSummary {
  kind: TransferKind
  activeCount: number
  failedCount: number
  /** Share of the active bytes moved, 0 to 1; null when no active transfer has a known size. */
  progress: number | null
}

export function isActiveTransfer(transfer: TransferTask): boolean {
  return transfer.status === "pending" || transfer.status === "transferring"
}

/**
 * Sums up the top-level transfers for the title bar button. While any is
 * active only the active ones count, so the icon shows what is moving now
 * rather than what moved earlier.
 */
export function summarizeTransfers(transfers: TransferTask[]): TransferSummary {
  const active = transfers.filter(isActiveTransfer)
  const counted = active.length > 0 ? active : transfers
  const directions = new Set(counted.map((transfer) => transfer.direction))
  const [onlyDirection] = directions
  const kind: TransferKind = directions.size === 1 ? onlyDirection : "mixed"

  // A delete counts items, a server command nothing, and a byte transfer
  // bytes, so only byte transfers with a known total go into the share.
  let total = 0
  let moved = 0
  for (const transfer of active) {
    if (
      transfer.direction === "delete" ||
      isRemoteOperation(transfer) ||
      transfer.sizeUnknown ||
      transfer.fileSize <= 0
    ) {
      continue
    }
    total += transfer.fileSize
    moved += Math.min(transfer.transferred, transfer.fileSize)
  }

  return {
    kind,
    activeCount: active.length,
    failedCount: transfers.filter((transfer) => transfer.status === "failed").length,
    progress: total > 0 ? moved / total : null,
  }
}
