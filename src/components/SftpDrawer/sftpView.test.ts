import { describe, expect, it } from "vitest"

import {
  applySftpView,
  DEFAULT_SFTP_VIEW_PREFERENCES,
  formatOctalMode,
  loadSftpViewPreferences,
  nextSftpSort,
  parseOctalMode,
  saveSftpViewPreferences,
  sortSftpEntries,
} from "@/components/SftpDrawer/sftpView"
import type { SftpDirectoryEntry } from "@/components/SftpDrawer/types"

function entry(name: string, overrides: Partial<SftpDirectoryEntry> = {}): SftpDirectoryEntry {
  return { name, path: `/srv/${name}`, isDir: false, isSymlink: false, ...overrides }
}

const entries = [
  entry("notes.txt", { size: 300, modifiedAt: 3 }),
  entry("logs", { isDir: true, modifiedAt: 9 }),
  entry("file10.log", { size: 20, modifiedAt: 1 }),
  entry(".env", { size: 5, modifiedAt: 2 }),
  entry("File2.log", { size: 100, modifiedAt: 4 }),
  entry("app", { isDir: true, modifiedAt: 5 }),
]

const names = (list: SftpDirectoryEntry[]) => list.map((item) => item.name)

describe("sortSftpEntries", () => {
  it("sorts names naturally and case-insensitively with folders first", () => {
    expect(names(sortSftpEntries(entries, { column: "name", direction: "asc" }))).toEqual([
      "app",
      "logs",
      ".env",
      "File2.log",
      "file10.log",
      "notes.txt",
    ])
  })

  it("keeps folders first when descending", () => {
    expect(names(sortSftpEntries(entries, { column: "name", direction: "desc" }))).toEqual([
      "logs",
      "app",
      "notes.txt",
      "file10.log",
      "File2.log",
      ".env",
    ])
  })

  it("sorts by size and by modification time", () => {
    expect(names(sortSftpEntries(entries, { column: "size", direction: "desc" })).slice(2)).toEqual(
      ["notes.txt", "File2.log", "file10.log", ".env"]
    )
    expect(names(sortSftpEntries(entries, { column: "modified", direction: "asc" }))).toEqual([
      "app",
      "logs",
      "file10.log",
      ".env",
      "notes.txt",
      "File2.log",
    ])
  })

  it("groups files by extension for the kind column, names breaking ties", () => {
    expect(names(sortSftpEntries(entries, { column: "kind", direction: "asc" })).slice(2)).toEqual([
      ".env",
      "File2.log",
      "file10.log",
      "notes.txt",
    ])
  })

  it("does not mutate the listing", () => {
    const before = names(entries)
    sortSftpEntries(entries, { column: "size", direction: "asc" })
    expect(names(entries)).toEqual(before)
  })
})

describe("applySftpView", () => {
  it("drops dotfiles when hidden files are off", () => {
    expect(
      names(applySftpView(entries, { ...DEFAULT_SFTP_VIEW_PREFERENCES, showHidden: false }))
    ).toEqual(["app", "logs", "File2.log", "file10.log", "notes.txt"])
  })
})

describe("nextSftpSort", () => {
  it("flips the direction of the sorted column and resets for another", () => {
    expect(nextSftpSort({ column: "name", direction: "asc" }, "name")).toEqual({
      column: "name",
      direction: "desc",
    })
    expect(nextSftpSort({ column: "name", direction: "desc" }, "size")).toEqual({
      column: "size",
      direction: "asc",
    })
  })
})

describe("view preferences", () => {
  it("round-trips through storage and ignores malformed values", () => {
    const store = new Map<string, string>()
    const storage = {
      getItem: (key: string) => store.get(key) ?? null,
      setItem: (key: string, value: string) => void store.set(key, value),
    }
    expect(loadSftpViewPreferences(storage)).toEqual(DEFAULT_SFTP_VIEW_PREFERENCES)

    const preferences = { sort: { column: "size", direction: "desc" }, showHidden: false } as const
    saveSftpViewPreferences(preferences, storage)
    expect(loadSftpViewPreferences(storage)).toEqual(preferences)

    store.set("tterm.sftp.view", '{"sort":{"column":"bogus","direction":"up"},"showHidden":1}')
    expect(loadSftpViewPreferences(storage)).toEqual(DEFAULT_SFTP_VIEW_PREFERENCES)
    store.set("tterm.sftp.view", "not json")
    expect(loadSftpViewPreferences(storage)).toEqual(DEFAULT_SFTP_VIEW_PREFERENCES)
  })
})

describe("octal modes", () => {
  it("parses three or four octal digits only", () => {
    expect(parseOctalMode("644")).toBe(0o644)
    expect(parseOctalMode(" 2755 ")).toBe(0o2755)
    expect(parseOctalMode("64")).toBeNull()
    expect(parseOctalMode("688")).toBeNull()
    expect(parseOctalMode("07777 ")).toBeNull()
    expect(parseOctalMode("rwx")).toBeNull()
  })

  it("formats modes with at least three digits", () => {
    expect(formatOctalMode(0o644)).toBe("644")
    expect(formatOctalMode(0o7)).toBe("007")
    expect(formatOctalMode(0o4755)).toBe("4755")
  })
})
