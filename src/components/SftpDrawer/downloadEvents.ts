export interface DownloadProgressEvent {
  localPath: string
  progress: number
  remotePath: string
  total: number
  transferred: number
  transferId: string
  resumedFrom?: number
  parallelism?: number
  completedChunks?: number
  chunkCount?: number
}

export interface DownloadItemStartEvent {
  batchId: string
  fileName: string
  fileSize: number
  localPath: string
  remotePath: string
  transferId: string
}

export interface DownloadItemCompleteEvent {
  cancelled: boolean
  error?: string
  localPath: string
  remotePath: string
  success: boolean
  transferId: string
  skipped?: boolean
}

export interface DownloadBatchCompleteEvent {
  batchId: string
  cancelled: boolean
  error?: string
  total: number
  transferred: number
  /** Bytes of files the run skipped because the local copy was current. */
  skipped?: number
  /** Bytes of files the run resumed from a previous partial attempt. */
  resumed?: number
}
