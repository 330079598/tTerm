import type { Terminal } from "@xterm/xterm"

interface OscLinkData {
  id?: string
  uri: string
}

interface LinkMarker {
  readonly line: number
}

interface OscLinkEntry {
  id: number
  data: OscLinkData
  lines: LinkMarker[]
}

interface OscLinkServiceInternals {
  registerLink: (data: OscLinkData) => number
  _removeMarkerFromLink: (entry: OscLinkEntry, marker: LinkMarker) => void
  _dataByLinkId: Map<number, OscLinkEntry>
  _bufferService: { buffer: { ybase: number; y: number } }
}

function getOscLinkService(term: Terminal): OscLinkServiceInternals | null {
  const service = (term as unknown as { _core?: { _oscLinkService?: OscLinkServiceInternals } })
    ._core?._oscLinkService
  if (
    !service ||
    typeof service.registerLink !== "function" ||
    typeof service._removeMarkerFromLink !== "function" ||
    !(service._dataByLinkId instanceof Map) ||
    !service._bufferService?.buffer
  ) {
    return null
  }
  return service
}

/**
 * Stops OSC 8 hyperlinks without an `id` from piling up while a program
 * redraws them in place.
 *
 * xterm 6 registers every id-less link as a new entry with its own buffer
 * marker, and frees it only once its line leaves the scrollback. A TUI that
 * repaints a line holding a link (a status line, a spinner next to a file
 * name) adds an entry of about 3 KB on every frame, which with a large or
 * unlimited scrollback is never freed.
 *
 * An id-less link now reuses the entry of the same URI that already has a
 * marker on the same line of the same buffer, so entries are bounded by the
 * distinct links on screen. Hover ranges come from adjacent cells sharing an
 * entry, so the only visible change is that two id-less links to the same URI
 * placed directly next to each other underline together.
 *
 * It patches xterm 6 private members on the instance; if any are missing it
 * is a no-op and xterm keeps its default behaviour.
 */
export function installOscLinkDedupe(term: Terminal): void {
  const service = getOscLinkService(term)
  if (!service) return

  const registerLink = service.registerLink.bind(service)
  const removeMarkerFromLink = service._removeMarkerFromLink.bind(service)
  const entriesByUri = new Map<string, Set<OscLinkEntry>>()
  // Markers count lines within their own buffer, so a line number alone
  // would match a link in the other buffer.
  const entryBuffers = new WeakMap<OscLinkEntry, object>()

  service.registerLink = (data) => {
    if (data.id !== undefined) return registerLink(data)
    const buffer = service._bufferService.buffer
    const line = buffer.ybase + buffer.y
    const entries = entriesByUri.get(data.uri)
    if (entries) {
      for (const entry of entries) {
        if (entryBuffers.get(entry) === buffer && entry.lines.some((m) => m.line === line)) {
          return entry.id
        }
      }
    }

    const id = registerLink(data)
    const entry = service._dataByLinkId.get(id)
    if (entry) {
      entryBuffers.set(entry, buffer)
      if (entries) entries.add(entry)
      else entriesByUri.set(data.uri, new Set([entry]))
    }
    return id
  }

  // xterm drops an entry here once its last marker is disposed, which is
  // when this index must forget it too.
  service._removeMarkerFromLink = (entry, marker) => {
    removeMarkerFromLink(entry, marker)
    if (entry.data.id !== undefined || entry.lines.length > 0) return
    const entries = entriesByUri.get(entry.data.uri)
    if (!entries) return
    entries.delete(entry)
    if (entries.size === 0) entriesByUri.delete(entry.data.uri)
  }
}
