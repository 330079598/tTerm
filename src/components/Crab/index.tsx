import "@/components/Crab/crab.css"

import arm from "@/assets/crab/arm.svg"
import body from "@/assets/crab/body.svg"
import eyes from "@/assets/crab/eyes.svg"
import hit from "@/assets/crab/hit.svg"
import jawInner from "@/assets/crab/jaw-inner.svg"
import jawOuter from "@/assets/crab/jaw-outer.svg"
import smallBody from "@/assets/crab/small-body.svg"
import smallClaw from "@/assets/crab/small-claw.svg"
import smallHit from "@/assets/crab/small-hit.svg"
import { cn } from "@/lib/utils"

/** What the crab does: pinch while something loads, knock while the user is asked for something. */
export type CrabMotion = "pinch" | "knock"

/** Below this height the full drawing's lines and jaws blur, so the small one is drawn. */
const SMALL_BELOW = 32

// Width over height of each drawing's viewBox (scripts/app-icon/crab.mjs).
const FULL_RATIO = 240 / 176
const SMALL_RATIO = 28 / 18

interface CrabProps {
  motion: CrabMotion
  /** Its height in pixels; it is wider than tall. */
  height: number
  className?: string
}

/**
 * tTerm's crab, pinching or knocking. It is decorative: the text beside it
 * says what is loading or what is asked.
 */
export function Crab({ motion, height, className }: CrabProps) {
  const small = height < SMALL_BELOW
  const width = Math.round(height * (small ? SMALL_RATIO : FULL_RATIO))
  return (
    <span
      className={cn("crab", className)}
      data-motion={motion}
      data-size={small ? "small" : "full"}
      style={{ width, height }}
      aria-hidden="true"
    >
      <span className="crab-figure">
        <CrabClaw small={small} knocks={motion === "knock"} />
        <CrabClaw small={small} knocks={motion === "knock"} mirrored />
        <img src={small ? smallBody : body} alt="" draggable={false} />
        {!small && <img className="crab-eyes" src={eyes} alt="" draggable={false} />}
      </span>
    </span>
  )
}

/** One arm and claw, drawn on the left; the right one is mirrored. */
function CrabClaw({
  small,
  knocks,
  mirrored,
}: {
  small: boolean
  knocks: boolean
  mirrored?: boolean
}) {
  return (
    <span className="crab-side" data-mirrored={mirrored || undefined}>
      <span className="crab-claw">
        {small ? (
          <img src={smallClaw} alt="" draggable={false} />
        ) : (
          <>
            <img src={arm} alt="" draggable={false} />
            <img className="crab-jaw-outer" src={jawOuter} alt="" draggable={false} />
            <img className="crab-jaw-inner" src={jawInner} alt="" draggable={false} />
          </>
        )}
        {knocks && (
          <img className="crab-hit" src={small ? smallHit : hit} alt="" draggable={false} />
        )}
      </span>
    </span>
  )
}
