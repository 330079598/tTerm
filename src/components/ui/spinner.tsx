import "@/components/ui/spinner.css"

import { useSyncExternalStore } from "react"

import { Crab } from "@/components/Crab"
import { getSpinnerStyle, subscribeSpinnerStyle } from "@/lib/spinnerStyle"
import type { TabAgentWorkingStyle } from "@/lib/tabAgentIndicator"
import { cn } from "@/lib/utils"

interface SpinnerProps {
  /** Its width and height in pixels; the dots run a little wider and the crab half as wide again. */
  size?: number
  /** A style to show instead of the one the settings pick. */
  variant?: TabAgentWorkingStyle
  className?: string
}

/**
 * Shows that something is loading, in the text color and in the animation
 * picked for an AI agent at work (Settings > Appearance). It is decorative:
 * the control or text beside it says what is loading.
 */
export function Spinner({ size = 16, variant, className }: SpinnerProps) {
  const picked = useSyncExternalStore(subscribeSpinnerStyle, getSpinnerStyle)
  const style = variant ?? picked
  if (style === "crab") return <Crab motion="pinch" height={size} className={className} />
  return (
    <span
      className={cn("spinner", className)}
      data-style={style}
      style={{ fontSize: size }}
      aria-hidden="true"
    >
      {/* Each dot bounces on its own, so each needs an element. */}
      {style === "dots" && (
        <>
          <i />
          <i />
          <i />
        </>
      )}
    </span>
  )
}
