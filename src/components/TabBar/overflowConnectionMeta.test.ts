import { describe, expect, it } from "vitest"

import { getOverflowConnectionMeta } from "@/components/TabBar/overflowConnectionMeta"
import type { Tab } from "@/types/tab"

const remoteFile = { sourceTabId: "ssh-1", path: "/etc/hosts", fileName: "hosts", size: 10 }

function tab(overrides: Partial<Tab>): Tab {
  return { id: "tab-1", title: "prod", type: "ssh", isActive: false, ...overrides }
}

describe("getOverflowConnectionMeta", () => {
  it("shows the host a connection reaches, with its port when it is not 22", () => {
    expect(getOverflowConnectionMeta(tab({ connection: { type: "ssh", host: " web " } }))).toEqual({
      primary: "web",
    })
    expect(
      getOverflowConnectionMeta(tab({ connection: { type: "ssh", host: "web", port: 2222 } }))
    ).toEqual({ primary: "web:2222" })
    expect(getOverflowConnectionMeta(tab({ type: "terminal" }))).toEqual({ primary: undefined })
  })

  it("shows a remote file's connection first and its host after", () => {
    const editor = tab({
      type: "remote-file-editor",
      remoteFile: { ...remoteFile, host: "10.0.0.5", profileName: "Production" },
    })
    expect(getOverflowConnectionMeta(editor)).toEqual({
      primary: "Production",
      secondary: "10.0.0.5",
    })
  })

  it("shows a remote file's host once when its connection has no other name", () => {
    const named = tab({
      type: "remote-file-editor",
      remoteFile: { ...remoteFile, host: "web", connectionLabel: "web" },
    })
    expect(getOverflowConnectionMeta(named)).toEqual({ primary: "web", secondary: undefined })
    const unnamed = tab({ type: "remote-file-editor", remoteFile: { ...remoteFile, host: "web" } })
    expect(getOverflowConnectionMeta(unnamed)).toEqual({ primary: "web" })
  })
})
