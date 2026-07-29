import { describe, expect, it } from "vitest"

import {
  isTerminalConnectionUnavailable,
  resolveSavedPasswordInjectionTargets,
} from "@/components/TTermApp/ttermAppUtils"

describe("isTerminalConnectionUnavailable", () => {
  it("keeps live input during terminal visibility and reconnect transitions", () => {
    expect(isTerminalConnectionUnavailable(null)).toBe(false)
    expect(isTerminalConnectionUnavailable(undefined)).toBe(false)
    expect(isTerminalConnectionUnavailable("connecting")).toBe(false)
    expect(isTerminalConnectionUnavailable("connected")).toBe(false)
  })

  it("stops live input for a confirmed unavailable terminal", () => {
    expect(isTerminalConnectionUnavailable("disconnected")).toBe(true)
    expect(isTerminalConnectionUnavailable("error")).toBe(true)
  })
})

describe("resolveSavedPasswordInjectionTargets", () => {
  const source = { tabId: "source", sessionNonce: 1 }
  const targets = [
    { tabId: "ready", sessionNonce: 2 },
    { tabId: "not-prompting", sessionNonce: 3 },
    { tabId: "stale", sessionNonce: 4 },
  ]

  it("includes only linked terminals waiting for a saved password", () => {
    const prompts = new Map([
      ["ready", 2],
      ["stale", 3],
    ])

    expect(resolveSavedPasswordInjectionTargets(source, true, targets, prompts)).toEqual([
      source,
      targets[0],
    ])
  })

  it("injects only into the current terminal when live input is not active", () => {
    expect(
      resolveSavedPasswordInjectionTargets(
        source,
        false,
        targets,
        new Map(targets.map((target) => [target.tabId, target.sessionNonce]))
      )
    ).toEqual([source])
  })
})
