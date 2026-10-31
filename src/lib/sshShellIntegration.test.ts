import { describe, expect, it } from "vitest"

import {
  announcesShellIntegrationFallback,
  shellIntegrationChoice,
  shellIntegrationPayload,
  sshShellIntegrationEnabled,
} from "@/lib/sshShellIntegration"

describe("shellIntegrationChoice", () => {
  it("round-trips through the stored value", () => {
    for (const value of [undefined, true, false]) {
      expect(shellIntegrationPayload(shellIntegrationChoice(value))).toBe(value)
    }
  })
})

describe("sshShellIntegrationEnabled", () => {
  it("follows the setting unless the connection chose", () => {
    expect(sshShellIntegrationEnabled(undefined, true)).toBe(true)
    expect(sshShellIntegrationEnabled(undefined, false)).toBe(false)
    expect(sshShellIntegrationEnabled(false, true)).toBe(false)
    expect(sshShellIntegrationEnabled(true, false)).toBe(true)
  })
})

describe("announcesShellIntegrationFallback", () => {
  it("stays quiet about the host's own answers unless the connection turned it on", () => {
    for (const reason of [
      { code: "shell", shell: "tcsh" },
      { code: "noReply" },
      { code: "writeFailed" },
      { code: "path" },
    ] as const) {
      expect(announcesShellIntegrationFallback(undefined, reason)).toBe(false)
      expect(announcesShellIntegrationFallback(true, reason)).toBe(true)
    }
  })

  it("tells about failures a reconnect may not meet again", () => {
    for (const code of ["rejected", "timedOut", "failed"] as const) {
      expect(announcesShellIntegrationFallback(undefined, { code })).toBe(true)
    }
  })
})
