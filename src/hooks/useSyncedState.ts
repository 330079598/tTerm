import { useState } from "react"

/**
 * Local, editable state that resets whenever `source` changes (compared with
 * `Object.is`). The reset happens during render, so the component never
 * paints a frame with the stale draft — unlike syncing in an effect.
 */
export function useSyncedState<T>(source: T): [T, React.Dispatch<React.SetStateAction<T>>] {
  const [state, setState] = useState(source)
  const [prevSource, setPrevSource] = useState(source)
  if (!Object.is(source, prevSource)) {
    setPrevSource(source)
    setState(source)
  }
  return [state, setState]
}
