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

function setup(remote: boolean, hostname: string | null = "web01") {
  term = new Terminal({ cols: 80, rows: 24, allowProposedApi: true })
  const onCwd = vi.fn()
  const onRemoteCwd = vi.fn()
  const onRemoteAwayHost = vi.fn()
  const reports = watchCwdReports(term, {
    tabId: "tab",
    sessionNonce: 1,
    isRemote: () => remote,
    onCwd,
    onRemoteCwd,
    remoteHostname: () => hostname,
    onRemoteAwayHost,
  })
  const write = (data: string) => new Promise<void>((resolve) => term!.write(data, resolve))
  return { onCwd, onRemoteCwd, onRemoteAwayHost, reports, write }
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

  it("ignores OSC 9;9 over SSH", async () => {
    const { onRemoteCwd, write } = setup(true)
    await write('\x1b]9;9;"C:\\Users"\x1b\\')
    expect(onRemoteCwd).not.toHaveBeenCalled()
  })

  it("follows reports naming the connected host, however fully", async () => {
    const { onRemoteCwd, onRemoteAwayHost, write } = setup(true)
    await write("\x1b]7;file://WEB01.example.com/srv\x1b\\")
    await write("\x1b]7;file://localhost/tmp\x1b\\")
    expect(onRemoteCwd.mock.calls).toEqual([["/srv"], ["/tmp"]])
    expect(onRemoteAwayHost).not.toHaveBeenCalled()
  })

  it("pauses while another host reports and resumes when the connected one does", async () => {
    const { onRemoteCwd, onRemoteAwayHost, write } = setup(true)
    await write("\x1b]7;file:///srv\x1b\\")
    await write("\x1b]7;file://db01/etc\x1b\\")
    await write("\x1b]7;file://db01/var\x1b\\")
    expect(onRemoteCwd.mock.calls).toEqual([["/srv"]])
    expect(onRemoteAwayHost.mock.calls).toEqual([["db01"]])

    await write("\x1b]7;file:///srv\x1b\\")
    expect(onRemoteAwayHost).toHaveBeenLastCalledWith(null)
    expect(onRemoteCwd.mock.calls).toEqual([["/srv"]])
  })

  it("takes the first host a report names when the host's name is not known", async () => {
    const { onRemoteCwd, onRemoteAwayHost, write } = setup(true, null)
    await write("\x1b]7;file://web01/srv\x1b\\")
    await write("\x1b]7;file://db01/etc\x1b\\")
    expect(onRemoteCwd.mock.calls).toEqual([["/srv"]])
    expect(onRemoteAwayHost.mock.calls).toEqual([["db01"]])
  })

  it("clears the remote directory and host when the session goes", async () => {
    const { onRemoteCwd, onRemoteAwayHost, reports, write } = setup(true)
    await write("\x1b]7;file:///var/log\x1b\\")
    await write("\x1b]7;file://db01/etc\x1b\\")
    reports.dispose()
    expect(onRemoteCwd).toHaveBeenLastCalledWith(null)
    expect(onRemoteAwayHost).toHaveBeenLastCalledWith(null)
  })
})
