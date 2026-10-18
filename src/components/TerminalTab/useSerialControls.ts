import { useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { useTranslation } from "react-i18next"

import type { SshConnectionProgress } from "@/components/TerminalTab/types"
import { toast } from "@/hooks/use-toast"

type SerialLine = "dtr" | "rts"

/** The levels last set on a port's lines; unknown until set, and again once it reopens. */
interface LineLevels {
  /** The progress the levels were set under; a reopened port reports new progress. */
  progress: SshConnectionProgress | null | undefined
  dtr: boolean | null
  rts: boolean | null
}

/** A serial tab's modem control lines, break and reset. */
export function useSerialControls(
  tabId: string,
  sessionNonce: number,
  connectionProgress: SshConnectionProgress | null | undefined
) {
  const { t } = useTranslation()
  const [levels, setLevels] = useState<LineLevels>({
    progress: connectionProgress,
    dtr: null,
    rts: null,
  })
  const current =
    levels.progress === connectionProgress ? levels : { dtr: null, rts: null, progress: null }

  const run = (command: string, args: Record<string, unknown>) =>
    invoke(command, { tabId, sessionNonce, ...args }).then(
      () => true,
      (error: unknown) => {
        toast({
          variant: "destructive",
          title: t("serialControls.failed"),
          description: String(error),
        })
        return false
      }
    )

  const toggleLine = (line: SerialLine) => {
    const level = current[line] !== true
    void run("set_serial_signals", { [line]: level }).then((ok) => {
      if (ok) setLevels({ ...current, progress: connectionProgress, [line]: level })
    })
  }

  const reset = () => {
    void run("pulse_serial_line", { line: "rts" }).then((ok) => {
      // The pulse leaves RTS released.
      if (ok) setLevels({ ...current, progress: connectionProgress, rts: false })
    })
  }

  const sendBreak = () => {
    void run("send_serial_break", {})
  }

  /** Turns the hex view on or off, reporting whether the port took it. */
  const setHexView = (hex: boolean) => run("set_serial_view", { hex })

  return { dtr: current.dtr, rts: current.rts, toggleLine, reset, sendBreak, setHexView }
}
