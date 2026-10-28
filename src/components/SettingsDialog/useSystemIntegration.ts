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
 * status, read once, and a change that is reported when it fails.
 */
export function useSystemIntegration({ status, set, args, failedTitle }: IntegrationCommands) {
  const [current, setCurrent] = useState<IntegrationStatus | null>(null)
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    invoke<IntegrationStatus>(status)
      .then(setCurrent)
      .catch((error: unknown) => console.error(`Failed to run ${status}:`, error))
  }, [status])

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

  return { status: current, busy, change }
}
