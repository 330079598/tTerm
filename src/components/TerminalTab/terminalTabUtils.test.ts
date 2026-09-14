import { describe, expect, it } from "vitest"

import {
  getConnectionStateLabel,
  getSshConnectionProgressLabel,
} from "@/components/TerminalTab/terminalTabUtils"
import type { ConnectionState, SshConnectionProgress } from "@/components/TerminalTab/types"

const t = (key: string, options?: Record<string, unknown>) => {
  const template = (options?.defaultValue as string | undefined) ?? key
  if (!options) return template
  return template.replace(/\{\{(\w+)\}\}/g, (_, name) => String(options[name] ?? ""))
}

describe("getConnectionStateLabel", () => {
  it("returns a label for every connection state", () => {
    const states: ConnectionState[] = [
      "connecting",
      "connected",
      "reconnecting",
      "disconnected",
      "error",
    ]

    for (const state of states) {
      expect(getConnectionStateLabel(state, t)).toBeTruthy()
    }
  })

  it("distinguishes reconnecting from the initial connect", () => {
    expect(getConnectionStateLabel("connecting", t)).toBe("Connecting")
    expect(getConnectionStateLabel("reconnecting", t)).toBe("Reconnecting")
  })
})

describe("getSshConnectionProgressLabel retry phases", () => {
  it("localizes the retrying status from structured fields", () => {
    const progress: SshConnectionProgress = {
      phase: "retrying",
      message: "Reconnecting in 3s (attempt 2)",
      reason: "SSH channel closed",
      retryAttempt: 2,
      retryDelaySecs: 3.4,
      retryMaxAttempts: 5,
    }

    expect(getSshConnectionProgressLabel(progress, t)).toBe(
      "Disconnected (SSH channel closed). Reconnecting in 3s (attempt 2/5)"
    )
  })

  it("localizes the exhausted status from structured fields", () => {
    const progress: SshConnectionProgress = {
      phase: "retry_exhausted",
      message: "Reconnect attempts exhausted (SSH channel closed)",
      reason: "SSH channel closed",
      retryMaxAttempts: 5,
    }

    expect(getSshConnectionProgressLabel(progress, t)).toBe(
      "Automatic reconnect failed after 5 attempts: SSH channel closed"
    )
  })

  it("still prefers the backend message for other phases", () => {
    const progress: SshConnectionProgress = {
      phase: "ready",
      message: "Connected to user@host:22",
    }

    expect(getSshConnectionProgressLabel(progress, t)).toBe("Connected to user@host:22")
  })
})
