import { useCallback, useEffect, useRef, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { useTranslation } from "react-i18next"

import { toast } from "@/hooks/use-toast"
import { lineEndingBytes, parseHexBytes } from "@/lib/serial"
import type { SerialLineEnding } from "@/types/serial"

export type SerialSendMode = "text" | "hex"

/** One thing the send bar sent, for its history. */
export interface SerialSendEntry {
  mode: SerialSendMode
  text: string
}

const HISTORY_LIMIT = 30
/** The shortest repeat interval, which keeps a slow line from falling behind. */
export const MIN_REPEAT_MS = 10

interface UseSerialSenderOptions {
  tabId: string
  sessionNonce: number
  lineEnding: SerialLineEnding
}

/**
 * Sending from a serial tab's send bar: text the way typing sends it, or
 * hex bytes as they are, once or over and over, with a history of what went.
 */
export function useSerialSender({ tabId, sessionNonce, lineEnding }: UseSerialSenderOptions) {
  const { t } = useTranslation()
  const [history, setHistory] = useState<SerialSendEntry[]>([])
  const [repeating, setRepeating] = useState(false)
  const timerRef = useRef<number | null>(null)

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
          return true
        }
        const parsed = parseHexBytes(entry.text)
        if (!parsed.ok) {
          fail(t("serialSend.invalidHex", { token: parsed.token }))
          return false
        }
        const data = [...parsed.bytes, ...(appendEnter ? lineEndingBytes(lineEnding) : [])]
        await invoke("write_serial_bytes", { tabId, sessionNonce, data })
        return true
      } catch (error) {
        fail(String(error))
        return false
      }
    },
    [fail, lineEnding, sessionNonce, t, tabId]
  )

  const remember = (entry: SerialSendEntry) =>
    setHistory((current) =>
      [
        entry,
        ...current.filter((item) => item.mode !== entry.mode || item.text !== entry.text),
      ].slice(0, HISTORY_LIMIT)
    )

  /** Sends `entry`, and again every `repeatMs` when that is set, until stopped. */
  const send = async (entry: SerialSendEntry, appendEnter: boolean, repeatMs?: number) => {
    if (!entry.text.trim() && !appendEnter) return
    stopRepeat()
    if (!(await sendOnce(entry, appendEnter))) return
    remember(entry)
    if (repeatMs === undefined) return
    setRepeating(true)
    timerRef.current = window.setInterval(
      () => void sendOnce(entry, appendEnter),
      Math.max(MIN_REPEAT_MS, repeatMs)
    )
  }

  return { history, repeating, send, stopRepeat }
}
