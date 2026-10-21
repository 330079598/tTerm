import { Channel, invoke } from "@tauri-apps/api/core"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"

import type {
  SftpDirectoryEntry,
  SftpDirectoryListing,
  SftpDrawerProps,
  SftpSearchEvent,
  SftpSearchSummary,
} from "@/components/SftpDrawer/types"

export interface SftpRecursiveSearch {
  root: string
  query: string
  status: "searching" | "done" | "failed"
  entries: SftpDirectoryEntry[]
  /** Only counted when the server searches over SFTP. */
  scannedFolders: number
  summary: SftpSearchSummary | null
  error: string | null
}

/** Where `path` sits under `root`: its folder relative to it, empty at the top. */
export function locationUnder(root: string, path: string) {
  const prefix = root === "/" ? "/" : `${root}/`
  const relative = path.startsWith(prefix) ? path.slice(prefix.length) : path
  const slash = relative.lastIndexOf("/")
  return slash === -1 ? "" : relative.slice(0, slash)
}

interface UseSftpRecursiveSearchOptions {
  tabId: string
  connection: SftpDrawerProps["connection"]
  listing: SftpDirectoryListing | null
  searchQuery: string
}

/**
 * A search of the current folder and every folder under it, run on the
 * server. Its results stand in for the listing until the query is cleared
 * or another folder is opened, which is also when a running search stops.
 */
export function useSftpRecursiveSearch({
  tabId,
  connection,
  listing,
  searchQuery,
}: UseSftpRecursiveSearchOptions) {
  const [search, setSearch] = useState<SftpRecursiveSearch | null>(null)
  // Events and answers from a search that has been replaced are dropped.
  const latestIdRef = useRef<string | null>(null)

  const currentPath = listing?.currentPath ?? null
  const active = search && search.root === currentPath && searchQuery.trim() !== "" ? search : null

  const stop = useCallback(() => {
    const searchId = latestIdRef.current
    if (searchId) void invoke("sftp_cancel_remote_operation", { operationId: searchId })
  }, [])

  const start = useCallback(
    async (regex: boolean, includeHidden: boolean) => {
      const query = searchQuery.trim()
      if (!currentPath || !query) return
      stop()
      const searchId = crypto.randomUUID()
      latestIdRef.current = searchId
      const root = currentPath
      setSearch({
        root,
        query,
        status: "searching",
        entries: [],
        scannedFolders: 0,
        summary: null,
        error: null,
      })

      const update = (change: (current: SftpRecursiveSearch) => SftpRecursiveSearch) => {
        if (latestIdRef.current !== searchId) return
        setSearch((current) => (current ? change(current) : current))
      }
      const onEvent = new Channel<SftpSearchEvent>()
      onEvent.onmessage = (event) => {
        if (event.kind === "matches") {
          const found = event.entries.map((entry) => ({
            ...entry,
            location: locationUnder(root, entry.path),
          }))
          update((current) => ({ ...current, entries: [...current.entries, ...found] }))
        } else {
          update((current) => ({ ...current, scannedFolders: event.scannedFolders }))
        }
      }

      try {
        const summary = await invoke<SftpSearchSummary>("sftp_search_entries", {
          tabId,
          connection,
          searchId,
          request: { root, query, regex, includeHidden },
          onEvent,
        })
        update((current) => ({ ...current, status: "done", summary }))
      } catch (invokeError) {
        update((current) => ({ ...current, status: "failed", error: String(invokeError) }))
      }
    },
    [connection, currentPath, searchQuery, stop, tabId]
  )

  const isRunning = search?.status === "searching"
  useEffect(() => {
    if (isRunning && !active) stop()
  }, [active, isRunning, stop])
  useEffect(() => stop, [stop])

  const shownListing = useMemo(
    () => (active && listing ? { ...listing, entries: active.entries } : listing),
    [active, listing]
  )

  return { search: active, shownListing, start, stop }
}
