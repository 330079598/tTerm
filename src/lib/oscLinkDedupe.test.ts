// @vitest-environment jsdom
import { afterEach, beforeAll, describe, expect, it } from "vitest"
import { Terminal } from "@xterm/xterm"

import { installOscLinkDedupe } from "@/lib/oscLinkDedupe"

interface LinkInternals {
  _core: {
    _oscLinkService: { _dataByLinkId: Map<number, unknown> }
    _linkProviderService: {
      linkProviders: {
        provideLinks: (y: number, callback: (links: { text: string }[] | undefined) => void) => void
      }[]
    }
  }
}

const URI = "https://example.com/src/main.tsx"
const OTHER_URI = "https://example.com/src/App.tsx"

let term: Terminal | null = null

beforeAll(() => {
  window.matchMedia ||= () =>
    ({ matches: false, addListener() {}, removeListener() {} }) as unknown as MediaQueryList
  window.ResizeObserver ||= class {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
})

afterEach(() => {
  term?.dispose()
  term = null
})

function setup(scrollback = 1000) {
  const container = document.createElement("div")
  document.body.appendChild(container)
  term = new Terminal({ cols: 80, rows: 10, scrollback, allowProposedApi: true })
  term.open(container)
  installOscLinkDedupe(term)
  return term
}

function write(t: Terminal, data: string) {
  return new Promise<void>((resolve) => t.write(data, resolve))
}

function link(uri: string, text: string, params = "") {
  return `\x1b]8;${params};${uri}\x07${text}\x1b]8;;\x07`
}

function linkCount(t: Terminal) {
  return (t as unknown as LinkInternals)._core._oscLinkService._dataByLinkId.size
}

function linksOnRow(t: Terminal, row: number) {
  const providers = (t as unknown as LinkInternals)._core._linkProviderService.linkProviders
  const texts: string[] = []
  for (const provider of providers) {
    provider.provideLinks(row, (links) => texts.push(...(links ?? []).map((l) => l.text)))
  }
  return texts
}

describe("installOscLinkDedupe", () => {
  it("keeps one entry for an id-less link redrawn in place", async () => {
    const t = setup()
    let frames = ""
    for (let i = 0; i < 500; i++) frames += `\r\x1b[2K${link(URI, "main.tsx")} tick ${i}`
    await write(t, frames)

    expect(linkCount(t)).toBe(1)
    expect(linksOnRow(t, 1)).toEqual([URI])
  })

  it("keeps separate entries on separate lines and for separate URIs", async () => {
    const t = setup()
    await write(t, `${link(URI, "main")} ${link(OTHER_URI, "app")}\r\n${link(URI, "main")}`)

    expect(linkCount(t)).toBe(3)
    expect(linksOnRow(t, 1)).toEqual([URI, OTHER_URI])
    expect(linksOnRow(t, 2)).toEqual([URI])
  })

  it("leaves links with an id to xterm", async () => {
    const t = setup()
    await write(t, `${link(URI, "a", "id=1")}\r\n${link(URI, "b", "id=2")}\r\n${link(URI, "c")}`)

    expect(linkCount(t)).toBe(3)
  })

  it("forgets an entry once its line leaves the scrollback", async () => {
    const t = setup(5)
    await write(t, link(URI, "old"))
    await write(t, "\r\n".repeat(30))
    expect(linkCount(t)).toBe(0)

    await write(t, link(URI, "new"))
    expect(linkCount(t)).toBe(1)
    const row = t.buffer.active.baseY + t.buffer.active.cursorY + 1
    expect(linksOnRow(t, row)).toEqual([URI])
  })

  it("does not share an entry between the normal and the alternate buffer", async () => {
    const t = setup()
    await write(t, link(URI, "normal"))
    await write(t, `\x1b[?1049h\x1b[H${link(URI, "alt")}`)
    expect(linkCount(t)).toBe(2)

    await write(t, "\x1b[?1049l")
    expect(linkCount(t)).toBe(1)
    expect(linksOnRow(t, 1)).toEqual([URI])
  })
})
