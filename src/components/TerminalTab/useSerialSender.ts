import { useCallback, useEffect, useRef, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { useTranslation } from "react-i18next"

import { echoSerialSend } from "@/components/TerminalTab/serialDisplay"
import { useSerialSendHistory } from "@/components/TerminalTab/useSerialSendHistory"
import { toast } from "@/hooks/use-toast"
import { useStableRef } from "@/hooks/useStableRef"
import { lineEndingBytes, parseHexBytes } from "@/lib/serial"
import { loadSerialSendEcho, saveSerialSendEcho, type SerialSendEntry } from "@/lib/serialSendStore"
import type { SerialLineEnding } from "@/types/serial"

/** The shortest repeat interval, which keeps a slow line from falling behind. */
export const MIN_REPEAT_MS = 10

interface UseSerialSenderOptions {
  tabId: string
  sessionNonce: number
  lineEnding: SerialLineEnding
  /** The key the connection's history and echo choice are kept under. */
  storageKey: string
}

/**
 * Sending from a serial tab's send bar: text the way typing sends it, or
 * hex bytes as they are, once or over and over, shown in the terminal when
 * asked, with a history of what went that the connection keeps.
 */
export function useSerialSender({
  tabId,
  sessionNonce,
  lineEnding,
  storageKey,
}: UseSerialSenderOptions) {
  const { t } = useTranslation()
  const { history, record, remove, clear } = useSerialSendHistory(storageKey)
  const [echo, setEchoState] = useState(() => loadSerialSendEcho(storageKey))
  const [repeating, setRepeating] = useState(false)
  const timerRef = useRef<number | null>(null)
  // Read by the repeat timer, which outlives renders.
  const echoRef = useStableRef(echo)

  const stopRepeat = useCallback(() => {
    if (timerRef.current !== null) window.clearInterval(timerRef.current)
    timerRef.current = null
    setRepeating(false)
  }, [])

  useEffect(() => stopRepeat, [stopRepeat, tabId, sessionNonce])

  const fail = useCallback(
    (description: string) => {
      stopRepeat()
      toast({ variant: "destructive", title: t("serialSend.failed"), description })
    },
    [stopRepeat, t]
  )

  /** Sends `entry` once; `false` when it couldn't, after saying why. */
  const sendOnce = useCallback(
    async (entry: SerialSendEntry, appendEnter: boolean): Promise<boolean> => {
      try {
        if (entry.mode === "text") {
          // Goes out like typing: the backend turns the Enter into the line
          // ending and the text into the device's charset.
          const data = entry.text + (appendEnter ? "\r" : "")
          await invoke("write_pty", { tabId, sessionNonce, data })
          if (echoRef.current) echoSerialSend(tabId, { text: data })
          return true
        }
        const parsed = parseHexBytes(entry.text)
        if (!parsed.ok) {
          fail(t("serialSend.invalidHex", { token: parsed.token }))
          return false
        }
        const data = [...parsed.bytes, ...(appendEnter ? lineEndingBytes(lineEnding) : [])]
        await invoke("write_serial_bytes", { tabId, sessionNonce, data })
        if (echoRef.current) echoSerialSend(tabId, { bytes: data })
        return true
      } catch (error) {
        fail(String(error))
        return false
      }
    },
    [echoRef, fail, lineEnding, sessionNonce, t, tabId]
  )

  /** Sends `entry`, and again every `repeatMs` when that is set, until stopped. */
  const send = async (entry: SerialSendEntry, appendEnter: boolean, repeatMs?: number) => {
    if (!entry.text.trim() && !appendEnter) return
    stopRepeat()
    if (!(await sendOnce(entry, appendEnter))) return
    // A bare Enter is not worth recalling.
    if (entry.text.trim()) void record(entry)
    if (repeatMs === undefined) return
    setRepeating(true)
    timerRef.current = window.setInterval(
      () => void sendOnce(entry, appendEnter),
      Math.max(MIN_REPEAT_MS, repeatMs)
    )
  }

  const setEcho = (next: boolean) => {
    setEchoState(next)
    saveSerialSendEcho(storageKey, next)
  }

  return {
    history,
    removeSend: remove,
    clearHistory: clear,
    echo,
    setEcho,
    repeating,
    send,
    stopRepeat,
  }
}
