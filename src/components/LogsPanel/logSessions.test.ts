import { describe, expect, it } from "vitest"

import {
  findTabLog,
  idRange,
  logBaseName,
  logFolder,
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
  tabId: "tab-1",
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

  it("finds a tab's newest log, from this run or a restored tab's earlier run", () => {
    const older = { ...ssh, id: "older", tabId: "tab-3" }
    const newer = { ...ssh, id: "newer", tabId: "tab-3" }
    const other = { ...ssh, id: "other", tabId: "tab-3", host: "10.0.0.9" }
    const sessions = [newer, other, older] // newest first, as listed
    const request = {
      tabId: "tab-3",
      sessionType: "ssh" as const,
      host: "10.0.0.8",
      port: 2222,
      username: "root",
    }

    expect(findTabLog(sessions, request, ["older"])?.id).toBe("older")
    expect(findTabLog(sessions, request, ["gone", "older"])?.id).toBe("older")
    expect(findTabLog(sessions, request, [])?.id).toBe("newer")
    expect(findTabLog(sessions, { ...request, host: "10.0.0.7" }, [])).toBeUndefined()
    // A local tab id reused from an earlier run is not trusted.
    const localLog = { ...local, id: "local", tabId: "tab-3" }
    expect(findTabLog([localLog], { tabId: "tab-3", sessionType: "local" }, [])).toBeUndefined()
    expect(findTabLog([localLog], { tabId: "tab-3", sessionType: "local" }, ["local"])?.id).toBe(
      "local"
    )
  })

  it("splits a session id into its folders and base name", () => {
    expect(logFolder("2026/10/prod-1")).toBe("2026/10")
    expect(logBaseName("2026/10/prod-1")).toBe("prod-1")
    expect(logFolder("prod-1")).toBe("")
    expect(logBaseName("prod-1")).toBe("prod-1")
  })

  it("picks the ids between a shift-click and the last click", () => {
    const ids = ["a", "b", "c", "d"]
    expect(idRange(ids, "b", "d")).toEqual(["b", "c", "d"])
    expect(idRange(ids, "d", "b")).toEqual(["b", "c", "d"])
    expect(idRange(ids, null, "c")).toEqual(["c"])
    expect(idRange(ids, "gone", "c")).toEqual(["c"])
  })
})
