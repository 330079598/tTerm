// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest"
import { Terminal } from "@xterm/xterm"

import { watchCwdReports } from "@/components/TerminalTab/cwdReports"

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}))

let term: Terminal | null = null

afterEach(() => {
  term?.dispose()
  term = null
})

function setup(remote: boolean) {
  term = new Terminal({ cols: 80, rows: 24, allowProposedApi: true })
  const onCwd = vi.fn()
  const onRemoteCwd = vi.fn()
  const reports = watchCwdReports(term, {
    tabId: "tab",
    sessionNonce: 1,
    isRemote: () => remote,
    onCwd,
    onRemoteCwd,
  })
  const write = (data: string) => new Promise<void>((resolve) => term!.write(data, resolve))
  return { onCwd, onRemoteCwd, reports, write }
}

describe("watchCwdReports", () => {
  it("sends a local shell's directory to onCwd only", async () => {
    const { onCwd, onRemoteCwd, write } = setup(false)
    await write("\x1b]7;file:///home/me\x1b\\")
    expect(onCwd).toHaveBeenCalledWith("/home/me")
    expect(onRemoteCwd).not.toHaveBeenCalled()
  })

  it("sends a remote shell's directory to onRemoteCwd once per change", async () => {
    const { onCwd, onRemoteCwd, write } = setup(true)
    await write("\x1b]7;file:///var/log\x1b\\")
    await write("\x1b]7;file:///var/log\x1b\\")
    await write("\x1b]7;file:///srv/my%25app\x1b\\")
    expect(onCwd).not.toHaveBeenCalled()
    expect(onRemoteCwd.mock.calls).toEqual([["/var/log"], ["/srv/my%app"]])
  })

  it("ignores OSC 9;9 and other hosts' reports over SSH", async () => {
    const { onRemoteCwd, write } = setup(true)
    await write('\x1b]9;9;"C:\\Users"\x1b\\')
    await write("\x1b]7;file://nested-host/etc\x1b\\")
    expect(onRemoteCwd).not.toHaveBeenCalled()
  })

  it("clears the remote directory when the session goes", async () => {
    const { onRemoteCwd, reports, write } = setup(true)
    await write("\x1b]7;file:///var/log\x1b\\")
    reports.dispose()
    expect(onRemoteCwd).toHaveBeenLastCalledWith(null)
  })
})
