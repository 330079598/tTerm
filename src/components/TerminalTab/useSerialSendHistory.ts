import { useCallback, useEffect, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { useTranslation } from "react-i18next"

import { toast } from "@/hooks/use-toast"
import type { SerialSend, SerialSendEntry } from "@/lib/serialSendStore"
import { onSyncApplied } from "@/lib/sync"

/**
 * A serial connection's send history, kept in the database so it lasts,
 * syncs and goes into backups; reloaded when a sync brings other devices'.
 */
export function useSerialSendHistory(connectionKey: string) {
  const { t } = useTranslation()
  const [history, setHistory] = useState<SerialSend[]>([])

  useEffect(() => {
    let current = true
    const load = () => {
      invoke<SerialSend[]>("list_serial_sends", { connectionKey })
        .then((sends) => {
          if (current) setHistory(sends)
        })
        .catch(() => {
          // Without the database the bar still sends; it just recalls nothing.
        })
    }
    load()
    const stop = onSyncApplied(["serialSends"], load)
    return () => {
      current = false
      stop()
    }
  }, [connectionKey])

  const run = useCallback(
    async (command: string, args: Record<string, unknown>) => {
      try {
        const sends = await invoke<SerialSend[] | null>(command, { connectionKey, ...args })
        setHistory(sends ?? [])
      } catch (error) {
        toast({
          variant: "destructive",
          title: t("serialSend.historyFailed"),
          description: String(error),
        })
      }
    },
    [connectionKey, t]
  )

  /** Puts what was sent first in the history, once. */
  const record = useCallback(
    (entry: SerialSendEntry) => run("record_serial_send", { mode: entry.mode, text: entry.text }),
    [run]
  )
  const remove = useCallback((id: string) => run("delete_serial_send", { id }), [run])
  const clear = useCallback(() => run("clear_serial_sends", {}), [run])

  return { history, record, remove, clear }
}
