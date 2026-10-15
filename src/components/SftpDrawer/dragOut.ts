import { invoke } from "@tauri-apps/api/core"
import { platform } from "@tauri-apps/plugin-os"

import type { SftpDirectoryEntry } from "@/components/SftpDrawer/types"

/** How a drag out of the window ended; mirrors `drag_out::DragOutResult`. */
export type DragOutResult =
  { kind: "dropped"; directory: string } | { kind: "cancelled" } | { kind: "unknownTarget" }

/** Mirrors `drag_out::DragOutItem`. */
interface DragOutItem {
  name: string
  isDir: boolean
}

// A drag out passes over tTerm's own window on its way, and on Windows it
// carries placeholder files the window's file drop would upload. Its events
// can arrive just after the command returns, hence the delay before clearing.
const SELF_DROP_SETTLE_MS = 500
let activeDragOuts = 0

/** Whether a window file drop right now comes from tTerm's own drag out. */
export function isDragOutActive(): boolean {
  return activeDragOuts > 0
}

/** Only Windows and macOS tell tTerm which folder a drag landed on. */
export function supportsDragOut(): boolean {
  try {
    const os = platform()
    return os === "windows" || os === "macos"
  } catch {
    return false
  }
}

/**
 * Starts a native drag of `entries` and resolves once it is known where they
 * were dropped. Call it while the mouse button that started the drag is held.
 */
export async function startDragOut(entries: SftpDirectoryEntry[]): Promise<DragOutResult> {
  const items: DragOutItem[] = entries.map((entry) => ({ name: entry.name, isDir: entry.isDir }))
  activeDragOuts += 1
  try {
    return await invoke<DragOutResult>("start_drag_out", { items })
  } finally {
    window.setTimeout(() => {
      activeDragOuts -= 1
    }, SELF_DROP_SETTLE_MS)
  }
}

/**
 * Has the file manager list `directory` again once the download into it has
 * ended, since File Explorer can miss changes right after a drop.
 */
export async function refreshDropFolder(directory: string): Promise<void> {
  try {
    await invoke("refresh_drop_folder", { directory })
  } catch (error) {
    console.warn("Failed to refresh the folder the files were dropped on:", error)
  }
}
