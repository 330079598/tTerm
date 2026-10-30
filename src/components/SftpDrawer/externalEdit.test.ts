import { describe, expect, it } from "vitest"

import {
  hasLocalChanges,
  needsAttention,
  upsertExternalEdit,
  type ExternalEdit,
} from "@/components/SftpDrawer/externalEdit"

const edit = (id: string, remotePath: string, status: ExternalEdit["status"]): ExternalEdit => ({
  id,
  tabId: "tab",
  remotePath,
  fileName: remotePath.split("/").pop() ?? "",
  localPath: `/cache/${id}/file`,
  status,
})

describe("upsertExternalEdit", () => {
  it("adds a new file in path order", () => {
    const list = upsertExternalEdit(
      [edit("b", "/srv/b.conf", "synced")],
      edit("a", "/etc/a.conf", "synced")
    )
    expect(list.map((item) => item.id)).toEqual(["a", "b"])
  })

  it("replaces a file's old state", () => {
    const list = upsertExternalEdit(
      [edit("a", "/etc/a.conf", "synced")],
      edit("a", "/etc/a.conf", "modified")
    )
    expect(list).toHaveLength(1)
    expect(list[0].status).toBe("modified")
  })
})

describe("changes", () => {
  it("counts every state but synced as local changes", () => {
    expect(hasLocalChanges(edit("a", "/a", "synced"))).toBe(false)
    expect(hasLocalChanges(edit("a", "/a", "uploading"))).toBe(true)
    expect(hasLocalChanges(edit("a", "/a", "failed"))).toBe(true)
  })

  it("asks for attention for changes waiting on the user", () => {
    expect(needsAttention([edit("a", "/a", "synced"), edit("b", "/b", "uploading")])).toBe(false)
    expect(needsAttention([edit("a", "/a", "synced"), edit("b", "/b", "failed")])).toBe(true)
  })
})
