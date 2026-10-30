import type { TFunction } from "i18next"
import { describe, expect, it } from "vitest"

import { describeQuitBlockers } from "@/hooks/useQuitGuard"

// Shows each key with the values it was given.
const t = ((key: string, options?: Record<string, unknown>) =>
  [key, ...Object.values(options ?? {})].join("|")) as unknown as TFunction

describe("describeQuitBlockers", () => {
  it("has a line for each thing a quit would end", () => {
    const text = describeQuitBlockers({ tunnels: 2, unuploadedFiles: ["a.conf"] }, t)
    expect(text).toBe("quitGuard.tunnels|2\n\nquitGuard.files|1|a.conf")
  })

  it("leaves out what is not at stake", () => {
    expect(describeQuitBlockers({ tunnels: 0, unuploadedFiles: ["a.conf"] }, t)).toBe(
      "quitGuard.files|1|a.conf"
    )
  })

  it("names the first files and counts the rest", () => {
    const files = ["1", "2", "3", "4", "5", "6", "7"]
    expect(describeQuitBlockers({ tunnels: 0, unuploadedFiles: files }, t)).toBe(
      "quitGuard.files|7|quitGuard.namesAndMore|1, 2, 3, 4, 5|2"
    )
  })
})
