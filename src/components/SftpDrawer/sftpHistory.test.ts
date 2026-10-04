import { describe, expect, it } from "vitest"

import {
  dropSftpHistoryEntry,
  EMPTY_SFTP_HISTORY,
  landSftpHistory,
  recordSftpVisit,
  SFTP_HISTORY_LIMIT,
  sftpHistoryTarget,
  type SftpHistory,
} from "@/components/SftpDrawer/sftpHistory"

function visit(...paths: string[]): SftpHistory {
  return paths.reduce(recordSftpVisit, EMPTY_SFTP_HISTORY)
}

describe("recordSftpVisit", () => {
  it("appends new directories and points at the latest", () => {
    expect(visit("/home", "/var", "/var/log")).toEqual({
      paths: ["/home", "/var", "/var/log"],
      index: 2,
    })
  })

  it("ignores reloads of the current directory", () => {
    const history = visit("/home", "/var")
    expect(recordSftpVisit(history, "/var")).toBe(history)
  })

  it("drops forward entries when navigating somewhere new", () => {
    const history = landSftpHistory(visit("/home", "/var", "/var/log"), 0, "/home")
    expect(recordSftpVisit(history, "/etc")).toEqual({ paths: ["/home", "/etc"], index: 1 })
  })

  it("keeps only the most recent entries", () => {
    const paths = Array.from({ length: SFTP_HISTORY_LIMIT + 5 }, (_, index) => `/d${index}`)
    const history = visit(...paths)
    expect(history.paths).toHaveLength(SFTP_HISTORY_LIMIT)
    expect(history.paths[0]).toBe("/d5")
    expect(history.index).toBe(SFTP_HISTORY_LIMIT - 1)
  })
})

describe("sftpHistoryTarget", () => {
  it("returns null at either end", () => {
    expect(sftpHistoryTarget(EMPTY_SFTP_HISTORY, -1)).toBeNull()
    const history = visit("/home", "/var")
    expect(sftpHistoryTarget(history, 1)).toBeNull()
    expect(sftpHistoryTarget(history, -1)).toBe(0)
    expect(sftpHistoryTarget(landSftpHistory(history, 0, "/home"), 1)).toBe(1)
  })
})

describe("landSftpHistory", () => {
  it("keeps forward entries when stepping back", () => {
    expect(landSftpHistory(visit("/home", "/var"), 0, "/home")).toEqual({
      paths: ["/home", "/var"],
      index: 0,
    })
  })

  it("stores the path the server resolved", () => {
    expect(landSftpHistory(visit("/link", "/var"), 0, "/real")).toEqual({
      paths: ["/real", "/var"],
      index: 0,
    })
  })

  it("falls back to a regular visit for an index that no longer exists", () => {
    expect(landSftpHistory(visit("/home"), 3, "/var")).toEqual({
      paths: ["/home", "/var"],
      index: 1,
    })
  })
})

describe("dropSftpHistoryEntry", () => {
  it("removes an entry behind the current one", () => {
    expect(dropSftpHistoryEntry(visit("/a", "/b", "/c"), 1)).toEqual({
      paths: ["/a", "/c"],
      index: 1,
    })
  })

  it("removes an entry ahead of the current one", () => {
    const history = landSftpHistory(visit("/a", "/b", "/c"), 0, "/a")
    expect(dropSftpHistoryEntry(history, 1)).toEqual({ paths: ["/a", "/c"], index: 0 })
  })

  it("never removes the directory on screen", () => {
    const history = visit("/a", "/b")
    expect(dropSftpHistoryEntry(history, 1)).toBe(history)
  })
})
