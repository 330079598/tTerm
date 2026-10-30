import { useEffect, useRef } from "react"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import type { TFunction } from "i18next"
import { useTranslation } from "react-i18next"

import type { useConfirmDialog } from "@/components/ui/app-dialog"

type Confirm = ReturnType<typeof useConfirmDialog>["confirm"]

/** What a quit would end; mirrors `QuitBlockers` in `background/exit_guard.rs`. */
export interface QuitBlockers {
  tunnels: number
  /** Names of files opened from SFTP whose changes were never uploaded. */
  unuploadedFiles: string[]
}

/** Files named in the prompt; the rest are counted. */
const NAMED_FILES = 5

/** The prompt's lines, one for each thing a quit would end. */
export function describeQuitBlockers(blockers: QuitBlockers, t: TFunction): string {
  const lines: string[] = []
  if (blockers.tunnels > 0) lines.push(t("quitGuard.tunnels", { count: blockers.tunnels }))
  const files = blockers.unuploadedFiles
  if (files.length > 0) {
    const named = files.slice(0, NAMED_FILES).join(", ")
    const more = files.length - NAMED_FILES
    lines.push(
      t("quitGuard.files", {
        count: files.length,
        names: more > 0 ? t("quitGuard.namesAndMore", { names: named, count: more }) : named,
      })
    )
  }
  return lines.join("\n\n")
}

/**
 * Asks before the app quits with something at stake: running port-forwarding
 * tunnels, or files opened from SFTP with changes never uploaded. The backend holds the exit and sends what is at
 * stake; the answer goes back as `confirm_quit_app` or `cancel_quit_prompt`.
 */
export function useQuitGuard(confirm: Confirm): void {
  const { t } = useTranslation()
  const confirmRef = useRef(confirm)
  const tRef = useRef(t)
  useEffect(() => {
    confirmRef.current = confirm
    tRef.current = t
  })

  useEffect(() => {
    let disposed = false
    let off: (() => void) | undefined
    void listen<QuitBlockers>("quit-requested", async (event) => {
      const t = tRef.current
      const quit = await confirmRef.current({
        title: t("quitGuard.title"),
        description: describeQuitBlockers(event.payload, t),
        confirmText: t("quitGuard.quit"),
        variant: "destructive",
        defaultAction: "cancel",
      })
      await invoke(quit ? "confirm_quit_app" : "cancel_quit_prompt").catch(console.error)
    }).then((unlisten) => {
      if (disposed) unlisten()
      else off = unlisten
    })
    return () => {
      disposed = true
      off?.()
    }
  }, [])
}
