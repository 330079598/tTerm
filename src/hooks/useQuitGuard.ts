import { useEffect, useRef } from "react"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { useTranslation } from "react-i18next"

import type { useConfirmDialog } from "@/components/ui/app-dialog"

type Confirm = ReturnType<typeof useConfirmDialog>["confirm"]

/** What a quit would end; mirrors `QuitBlockers` in `background/exit_guard.rs`. */
export interface QuitBlockers {
  tunnels: number
}

/**
 * Asks before the app quits with something at stake, such as running
 * port-forwarding tunnels. The backend holds the exit and sends what is at
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
        title: t("tunnels.quitConfirmTitle", { defaultValue: "Quit while tunnels are running?" }),
        description: t("tunnels.quitConfirmDescription", {
          count: event.payload.tunnels,
          defaultValue:
            "{{count}} port-forwarding tunnel(s) are running and will stop when tTerm quits.",
        }),
        confirmText: t("tunnels.quitConfirm", { defaultValue: "Quit" }),
        variant: "destructive",
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
