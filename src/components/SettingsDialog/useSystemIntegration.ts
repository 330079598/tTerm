import { useCallback, useEffect, useState } from "react"
import { invoke } from "@tauri-apps/api/core"

import { toast } from "@/hooks/use-toast"
import { toErrorMessage } from "@/lib/utils"

/** Whether one of tTerm's additions to the system is in place, as the backend reports it. */
export interface IntegrationStatus {
  /** Whether tTerm can add it on this system. */
  supported: boolean
  installed: boolean
}

interface IntegrationCommands {
  /** Reads the status. */
  status: string
  /** Changes it, taking `installed` and `args`, and returns the new status. */
  set: string
  args?: Record<string, unknown>
  /** The toast title when a change fails. */
  failedTitle: string
}

/**
 * One of tTerm's additions to the system, such as the `tterm` command: its
 * status, read once and again on `retry` when that failed, and a change that
 * is reported when it fails. `status` is null until it has been read.
 */
export function useSystemIntegration({ status, set, args, failedTitle }: IntegrationCommands) {
  const [current, setCurrent] = useState<IntegrationStatus | null>(null)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const read = useCallback(
    () =>
      invoke<IntegrationStatus>(status).then(
        (value) => {
          setCurrent(value)
          setLoadError(null)
        },
        (error: unknown) => setLoadError(toErrorMessage(error))
      ),
    [status]
  )

  useEffect(() => {
    void read()
  }, [read])

  const retry = useCallback(async () => {
    setBusy(true)
    try {
      await read()
    } finally {
      setBusy(false)
    }
  }, [read])

  const change = useCallback(
    async (installed: boolean) => {
      setBusy(true)
      try {
        setCurrent(await invoke<IntegrationStatus>(set, { installed, ...args }))
      } catch (error) {
        toast({ title: failedTitle, description: toErrorMessage(error), variant: "destructive" })
      } finally {
        setBusy(false)
      }
    },
    [args, failedTitle, set]
  )

  return { status: current, loadError, busy, change, retry }
}
