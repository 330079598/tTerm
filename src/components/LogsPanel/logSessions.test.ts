import { describe, expect, it } from "vitest"

import {
  type LogSession,
  logSessionTarget,
  logSessionTitle,
  matchesLogFilter,
} from "@/components/LogsPanel"

const ssh: LogSession = {
  id: "prod-10.0.0.8-20261010-101010-a1",
  sessionType: "ssh",
  profile: "prod api",
  host: "10.0.0.8",
  port: 2222,
  username: "root",
  startedAtMs: 1,
  modifiedAtMs: 1,
  rawBytes: 1,
  plainBytes: 1,
  hasRaw: true,
  hasPlain: true,
  recording: false,
}
const local: LogSession = { ...ssh, sessionType: "local", profile: "terminal", host: "local" }

describe("log sessions", () => {
  it("names SSH sessions by connection and local ones by the given label", () => {
    expect(logSessionTitle(ssh, "Local")).toBe("prod api")
    expect(logSessionTitle({ ...ssh, profile: "" }, "Local")).toBe("10.0.0.8")
    expect(logSessionTitle(local, "Local")).toBe("Local")
  })

  it("shows where an SSH session went", () => {
    expect(logSessionTarget(ssh)).toBe("root@10.0.0.8:2222")
    expect(logSessionTarget({ ...ssh, port: 22, username: "" })).toBe("10.0.0.8")
    expect(logSessionTarget(local)).toBeNull()
  })

  it("filters by connection, host or user", () => {
    expect(matchesLogFilter(ssh, "PROD")).toBe(true)
    expect(matchesLogFilter(ssh, "root")).toBe(true)
    expect(matchesLogFilter(ssh, "staging")).toBe(false)
    expect(matchesLogFilter(ssh, "  ")).toBe(true)
  })
})
