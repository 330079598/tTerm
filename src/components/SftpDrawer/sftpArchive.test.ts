import { describe, expect, it } from "vitest"

import {
  archiveFormatOf,
  commonParent,
  defaultArchiveName,
  defaultExtractDestination,
  withArchiveExtension,
} from "@/components/SftpDrawer/sftpArchive"

describe("archiveFormatOf", () => {
  it("prefers the longer suffix", () => {
    expect(archiveFormatOf({ name: "site.TAR.GZ", isDir: false })).toBe("tarGz")
    expect(archiveFormatOf({ name: "app.log.gz", isDir: false })).toBe("gzip")
    expect(archiveFormatOf({ name: "a.tgz", isDir: false })).toBe("tarGz")
    expect(archiveFormatOf({ name: "a.7z", isDir: false })).toBe("sevenZip")
  })

  it("ignores folders, other files and bare suffixes", () => {
    expect(archiveFormatOf({ name: "backup.zip", isDir: true })).toBeNull()
    expect(archiveFormatOf({ name: "notes.txt", isDir: false })).toBeNull()
    expect(archiveFormatOf({ name: ".zip", isDir: false })).toBeNull()
  })
})

describe("defaultExtractDestination", () => {
  it("unpacks an archive into a folder named after it", () => {
    expect(defaultExtractDestination("/srv/site.tar.gz", "tarGz")).toBe("/srv/site")
    expect(defaultExtractDestination("/site.zip", "zip")).toBe("/site")
  })

  it("unpacks a single compressed file beside it", () => {
    expect(defaultExtractDestination("/var/log/app.log.gz", "gzip")).toBe("/var/log")
    expect(defaultExtractDestination("/dump.xz", "xz")).toBe("/")
  })
})

describe("defaultArchiveName", () => {
  it("names one entry's archive after it and several after their folder", () => {
    expect(defaultArchiveName([{ name: "logs" }], "/srv", "tarGz")).toBe("logs.tar.gz")
    expect(defaultArchiveName([{ name: "a" }, { name: "b" }], "/srv/app", "zip")).toBe("app.zip")
    expect(defaultArchiveName([{ name: "a" }, { name: "b" }], "/", "tar")).toBe("archive.tar")
  })
})

describe("withArchiveExtension", () => {
  it("swaps the extension the old format gave", () => {
    expect(withArchiveExtension("logs.tar.gz", "tarGz", "zip")).toBe("logs.zip")
    expect(withArchiveExtension("custom.bin", "tarGz", "zip")).toBe("custom.bin")
  })
})

describe("commonParent", () => {
  it("finds the folder shared by every entry", () => {
    expect(commonParent([{ path: "/srv/a" }, { path: "/srv/b" }])).toBe("/srv")
    expect(commonParent([{ path: "/a" }])).toBe("/")
    expect(commonParent([{ path: "/srv/a" }, { path: "/srv/x/b" }])).toBeNull()
  })
})
