// @vitest-environment jsdom

import { cleanup, render } from "@testing-library/react"
import { afterEach, describe, expect, it } from "vitest"

import { Crab } from "@/components/Crab"
import { TabAgentBadge } from "@/components/TabAttentionDot"

afterEach(cleanup)

function crab(element: HTMLElement) {
  const found = element.querySelector<HTMLElement>(".crab")
  if (!found) throw new Error("no crab")
  return found
}

describe("Crab", () => {
  it("draws the small crab below 32 px and the full one from there", () => {
    const small = crab(render(<Crab motion="pinch" height={18} />).container)
    expect(small.dataset.size).toBe("small")
    expect(small.style.width).toBe("28px")
    cleanup()
    const full = crab(render(<Crab motion="pinch" height={88} />).container)
    expect(full.dataset.size).toBe("full")
    expect(full.style.width).toBe("120px")
    expect(full.querySelectorAll(".crab-jaw-outer")).toHaveLength(2)
  })

  it("draws the impact marks only when it knocks", () => {
    const pinch = render(<Crab motion="pinch" height={40} />).container
    expect(pinch.querySelector(".crab-hit")).toBeNull()
    cleanup()
    const knock = render(<Crab motion="knock" height={40} />).container
    expect(knock.querySelectorAll(".crab-hit")).toHaveLength(2)
  })
})

describe("TabAgentBadge", () => {
  it("pinches while the agent works and knocks while it waits", () => {
    const working = render(
      <TabAgentBadge activity="processing" style="crab" color="" label="" />
    ).container
    expect(crab(working).dataset.motion).toBe("pinch")
    cleanup()
    const waiting = render(
      <TabAgentBadge activity="waiting" style="crab" color="" label="" />
    ).container
    expect(crab(waiting).dataset.motion).toBe("knock")
  })
})
