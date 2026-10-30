/** Mirrors `EditStatus` in `src-tauri/src/sftp/internal/api/external/mod.rs`. */
export type ExternalEditStatus = "synced" | "modified" | "uploading" | "failed"

/** A remote file open in a program on this computer; mirrors `ExternalEdit`. */
export interface ExternalEdit {
  id: string
  tabId: string
  remotePath: string
  fileName: string
  localPath: string
  status: ExternalEditStatus
  error?: string | null
}

/** Mirrors `UploadOutcome`. */
export type ExternalUploadOutcome =
  { kind: "uploaded"; edit: ExternalEdit } | { kind: "remoteChanged" }

/** The backend's event for an open file that changed state. */
export const EXTERNAL_EDIT_EVENT = "sftp-external-edit"

/** Files larger than this are only downloaded to open once the user agrees. */
export const LARGE_EXTERNAL_FILE_BYTES = 50 * 1024 * 1024

/** Files larger than this show that they are downloading. */
export const SLOW_EXTERNAL_FILE_BYTES = 1024 * 1024

/** The list with `edit` in it, in place of its old state, sorted by path. */
export function upsertExternalEdit(edits: ExternalEdit[], edit: ExternalEdit): ExternalEdit[] {
  return [...edits.filter((existing) => existing.id !== edit.id), edit].sort((a, b) =>
    a.remotePath.localeCompare(b.remotePath)
  )
}

/** Whether the copy holds changes the server does not have. */
export function hasLocalChanges(edit: ExternalEdit): boolean {
  return edit.status !== "synced"
}

/** Whether the list needs the user's attention: changes not uploaded. */
export function needsAttention(edits: ExternalEdit[]): boolean {
  return edits.some((edit) => edit.status === "modified" || edit.status === "failed")
}
