/** Acknowledge right away once this many parsed bytes have accumulated. */
export const OUTPUT_ACK_BATCH_BYTES = 64 * 1024

/** Smaller remainders are acknowledged after this delay. */
export const OUTPUT_ACK_DELAY_MS = 20

/**
 * Batches acknowledgements of output bytes xterm.js has parsed. The backend
 * stops sending once about 1 MiB is outstanding, so acknowledging promptly
 * keeps a flood flowing, while batching keeps interactive output from costing
 * one IPC call per keystroke echo.
 */
export class OutputAcker {
  private pending = 0
  private timer: ReturnType<typeof setTimeout> | null = null
  private disposed = false

  constructor(private readonly send: (bytes: number) => void) {}

  /** Records `bytes` of channel output as parsed. */
  parsed(bytes: number) {
    if (this.disposed || bytes <= 0) return
    this.pending += bytes
    if (this.pending >= OUTPUT_ACK_BATCH_BYTES) {
      this.flush()
    } else if (this.timer === null) {
      this.timer = setTimeout(() => this.flush(), OUTPUT_ACK_DELAY_MS)
    }
  }

  dispose() {
    this.disposed = true
    this.clearTimer()
    this.pending = 0
  }

  private flush() {
    this.clearTimer()
    if (this.pending === 0) return
    const bytes = this.pending
    this.pending = 0
    this.send(bytes)
  }

  private clearTimer() {
    if (this.timer !== null) {
      clearTimeout(this.timer)
      this.timer = null
    }
  }
}
