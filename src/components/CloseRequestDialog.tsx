import React, { useEffect, useRef, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { type CloseBehavior, useConfig } from "@/contexts/ConfigContext"

type CloseAction = Exclude<CloseBehavior, "ask"> | "cancel"

interface CloseRequestDialogProps {
  runningTunnelCount: number
}

/**
 * Asks what closing the main window should do while `close_behavior` is
 * "ask". The backend holds the close and sends `close-requested`; the answer
 * goes back as `resolve_close_request`. A remembered answer is saved first so
 * the next close no longer asks.
 */
export const CloseRequestDialog: React.FC<CloseRequestDialogProps> = ({ runningTunnelCount }) => {
  const { t } = useTranslation()
  const { saveConfig } = useConfig()
  const [open, setOpen] = useState(false)
  const [remember, setRemember] = useState(false)
  const answeredRef = useRef(true)

  useEffect(() => {
    let disposed = false
    let off: (() => void) | undefined
    void listen("close-requested", () => {
      answeredRef.current = false
      setRemember(false)
      setOpen(true)
    }).then((unlisten) => {
      if (disposed) unlisten()
      else off = unlisten
    })
    return () => {
      disposed = true
      off?.()
    }
  }, [])

  const respond = async (action: CloseAction) => {
    // Escape and the buttons can both close the dialog; answer once.
    if (answeredRef.current) return
    answeredRef.current = true
    setOpen(false)
    if (remember && action !== "cancel") {
      await saveConfig({ close_behavior: action }).catch(console.error)
    }
    await invoke("resolve_close_request", { action }).catch(console.error)
  }

  return (
    <Dialog open={open} onOpenChange={(next) => !next && void respond("cancel")}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>
            {t("closeRequest.title", { defaultValue: "Close the tTerm window?" })}
          </DialogTitle>
          <DialogDescription className="whitespace-pre-line">
            {t("closeRequest.description", {
              defaultValue:
                "tTerm can keep running in the background with an icon in the system tray (the menu bar on macOS), so SSH sessions and port forwarding stay connected.",
            })}
            {runningTunnelCount > 0 &&
              "\n\n" +
                t("closeRequest.tunnelsRunning", {
                  count: runningTunnelCount,
                  defaultValue:
                    "{{count}} port-forwarding tunnel(s) are running. They keep running in the background and stop if tTerm quits.",
                })}
          </DialogDescription>
        </DialogHeader>
        <label className="flex cursor-pointer items-center gap-2 text-sm">
          <Checkbox checked={remember} onCheckedChange={setRemember} />
          {t("closeRequest.remember", {
            defaultValue: "Remember my choice (change it in Settings > General)",
          })}
        </label>
        <DialogFooter>
          <Button type="button" variant="outline" onClick={() => void respond("cancel")}>
            {t("common.cancel", { defaultValue: "Cancel" })}
          </Button>
          <Button type="button" variant="outline" onClick={() => void respond("quit")}>
            {t("closeRequest.quit", { defaultValue: "Quit" })}
          </Button>
          <Button
            type="button"
            data-dialog-initial-focus="true"
            onClick={() => void respond("tray")}
          >
            {t("closeRequest.background", { defaultValue: "Run in background" })}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
