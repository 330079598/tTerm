import { describe, expect, it } from "vitest"

import { isTerminalConnectionUnavailable } from "@/components/TTermApp/ttermAppUtils"

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
