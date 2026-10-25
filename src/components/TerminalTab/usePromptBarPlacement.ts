import { useLayoutEffect, useState, type RefObject } from "react"

import {
  promptBarPlacement,
  type PromptBarPlacement,
  type SurfaceRect,
} from "@/components/TerminalTab/passwordPromptLayout"

/**
 * Picks the corner for the saved-password bar that keeps the prompt line in
 * view, measuring again whenever the bar or the surface it sits in resizes.
 */
export function usePromptBarPlacement(
  barRef: RefObject<HTMLElement | null>,
  input: SurfaceRect | null
): PromptBarPlacement {
  const [placement, setPlacement] = useState<PromptBarPlacement>("bottom")

  useLayoutEffect(() => {
    const bar = barRef.current
    const surface = bar?.offsetParent
    if (!bar || !(surface instanceof HTMLElement)) return
    // The bar's size does not depend on its corner, so moving it cannot
    // change the answer and the two never chase each other.
    const update = () =>
      setPlacement(
        promptBarPlacement({
          surfaceWidth: surface.clientWidth,
          surfaceHeight: surface.clientHeight,
          barWidth: bar.offsetWidth,
          barHeight: bar.offsetHeight,
          input,
        })
      )
    update()
    const observer = new ResizeObserver(update)
    observer.observe(bar)
    observer.observe(surface)
    return () => observer.disconnect()
  }, [barRef, input])

  return placement
}
