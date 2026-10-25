// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest"
import { Terminal } from "@xterm/xterm"

import { watchOsReports } from "@/components/TerminalTab/osReports"

let term: Terminal | null = null

afterEach(() => {
  term?.dispose()
  term = null
})

function setup(remote: boolean) {
  term = new Terminal({ cols: 80, rows: 24, allowProposedApi: true })
  const onDistro = vi.fn()
  const reports = watchOsReports(term, { isRemote: () => remote, onDistro })
  const write = (data: string) => new Promise<void>((resolve) => term!.write(data, resolve))
  return { onDistro, reports, write }
}

describe("watchOsReports", () => {
  it("names the distribution a local WSL shell reports", async () => {
    const { onDistro, write } = setup(false)
    await write("\x1b]777;tterm-os;kali;debian;Kali GNU/Linux\x07")
    expect(onDistro).toHaveBeenCalledWith("kali")
  })

  it("shows generic Linux for a distribution without a logo", async () => {
    const { onDistro, write } = setup(false)
    await write("\x1b]777;tterm-os;someos;;Some OS\x07")
    expect(onDistro).toHaveBeenCalledWith("linux")
  })

  it("ignores reports over SSH and other OSC 777 payloads", async () => {
    const remote = setup(true)
    await remote.write("\x1b]777;tterm-os;ubuntu;debian;Ubuntu\x07")
    expect(remote.onDistro).not.toHaveBeenCalled()
    remote.reports.dispose()
    term?.dispose()

    const local = setup(false)
    await local.write("\x1b]777;notify;title;body\x07")
    expect(local.onDistro).not.toHaveBeenCalled()
  })
})
