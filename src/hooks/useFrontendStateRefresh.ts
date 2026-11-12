import { useEffect } from "react"

import { onFrontendStateReloaded } from "@/lib/frontendStateCache"

/** Rereads imported frontend state so later saves do not restore old values. */
export function useFrontendStateRefresh(refresh: () => void): void {
  useEffect(() => onFrontendStateReloaded(refresh), [refresh])
}
