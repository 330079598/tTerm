import { useCallback, useSyncExternalStore } from "react"

import type { SerialSettings } from "@/types/serial"

/** How a serial tab shows its session, which its control bar changes while it runs. */
export interface SerialDisplay {
  timestamps: boolean
  localEcho: boolean
  /** Received bytes show in hex; the backend makes the hex. */
  hex: boolean
}

// Kept outside React so the output and input handlers, which outlive
// renders, read what the bar last set.
const displays = new Map<string, SerialDisplay>()
const listeners = new Set<() => void>()

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

/** A serial tab's display: as last set from its bar, else as its connection says. */
export function getSerialDisplay(tabId: string, settings?: SerialSettings): SerialDisplay {
  let display = displays.get(tabId)
  if (!display) {
    display = {
      timestamps: settings?.timestamps === true,
      localEcho: settings?.localEcho === true,
      hex: settings?.hexView === true,
    }
    displays.set(tabId, display)
  }
  return display
}

export function setSerialDisplay(tabId: string, patch: Partial<SerialDisplay>) {
  displays.set(tabId, { ...getSerialDisplay(tabId), ...patch })
  for (const listener of listeners) listener()
}

/** Forgets a closed tab's display. */
export function clearSerialDisplay(tabId: string) {
  displays.delete(tabId)
}

/** A serial tab's display, for its control bar. */
export function useSerialDisplay(tabId: string, settings?: SerialSettings) {
  const display = useSyncExternalStore(subscribe, () => getSerialDisplay(tabId, settings))
  const update = useCallback(
    (patch: Partial<SerialDisplay>) => setSerialDisplay(tabId, patch),
    [tabId]
  )
  return [display, update] as const
}
