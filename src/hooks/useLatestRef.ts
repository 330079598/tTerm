import type { MutableRefObject } from "react"

/**
 * Keeps `ref.current` equal to the value from the latest render, for
 * callbacks that must read current props/state without being recreated.
 * Takes a `useRef` from the caller so exhaustive-deps still treats it as
 * stable.
 *
 * Unlike `useStableRef` (updated in an effect), this is written during
 * render, so a child's mount/layout effect that calls back into the owner
 * already sees the new value.
 */
export function useLatestRef<T>(ref: MutableRefObject<T>, value: T): void {
  ref.current = value
}
