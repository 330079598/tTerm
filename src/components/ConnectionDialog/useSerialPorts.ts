import { useCallback, useEffect, useState } from "react"
import { invoke } from "@tauri-apps/api/core"

import type { SerialPortEntry } from "@/types/serial"

/** The serial ports this machine has, listed when the dialog opens and on request. */
export function useSerialPorts() {
  const [ports, setPorts] = useState<SerialPortEntry[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const load = useCallback(() => {
    return invoke<SerialPortEntry[]>("list_serial_ports")
      .then((listed) => {
        setPorts(listed)
        setError(null)
      })
      .catch((reason: unknown) => setError(String(reason)))
      .finally(() => setLoading(false))
  }, [])

  useEffect(() => {
    void load()
  }, [load])

  const refresh = useCallback(() => {
    setLoading(true)
    void load()
  }, [load])

  return { ports, loading, error, refresh }
}
