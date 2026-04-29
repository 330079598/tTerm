import { useCallback, useMemo } from "react"
import type { Dispatch, SetStateAction } from "react"

import type {
  SftpContextMenuState,
  SftpDirectoryEntry,
  SftpDirectoryListing,
} from "@/components/SftpDrawer/types"

type UseSftpSelectionOptions = {
  activePath: string | null
  contextMenu: SftpContextMenuState | null
  listing: SftpDirectoryListing | null
  rangeEntries?: SftpDirectoryEntry[]
  selectedPaths: string[]
  setActivePath: (path: string | null) => void
  setSelectedPaths: Dispatch<SetStateAction<string[]>>
}

export function useSftpSelection({
  activePath,
  contextMenu,
  listing,
  rangeEntries,
  selectedPaths,
  setActivePath,
  setSelectedPaths,
}: UseSftpSelectionOptions) {
  const entryMap = useMemo(
    () => new Map((listing?.entries ?? []).map((entry) => [entry.path, entry])),
    [listing?.entries]
  )

  const selectedEntries = useMemo(
    () =>
      selectedPaths
        .map((path) => entryMap.get(path))
        .filter((entry): entry is SftpDirectoryEntry => Boolean(entry)),
    [entryMap, selectedPaths]
  )

  const activeEntry = useMemo(
    () => (activePath ? (entryMap.get(activePath) ?? null) : null),
    [activePath, entryMap]
  )

  const contextMenuEntry = useMemo(
    () => listing?.entries.find((entry) => entry.path === contextMenu?.entryPath) ?? null,
    [listing?.entries, contextMenu?.entryPath]
  )

  const breadcrumbs = useMemo(() => {
    const currentPath = listing?.currentPath ?? "/"
    if (currentPath === "/") {
      return [{ label: "/", path: "/" }]
    }

    const parts = currentPath.split("/").filter(Boolean)
    let cursor = ""
    const items = [{ label: "/", path: "/" }]
    parts.forEach((part) => {
      cursor = `${cursor}/${part}`
      items.push({ label: part, path: cursor })
    })
    return items
  }, [listing?.currentPath])

  const handleActivateEntry = useCallback(
    (path: string | null) => {
      setActivePath(path)
    },
    [setActivePath]
  )

  const buildSelectionRange = useCallback(
    (startPath: string, endPath: string) => {
      const entries = rangeEntries ?? listing?.entries ?? []
      const startIndex = entries.findIndex((entry) => entry.path === startPath)
      const endIndex = entries.findIndex((entry) => entry.path === endPath)

      if (startIndex === -1 || endIndex === -1) {
        return [endPath]
      }

      const [from, to] = startIndex <= endIndex ? [startIndex, endIndex] : [endIndex, startIndex]
      return entries.slice(from, to + 1).map((entry) => entry.path)
    },
    [listing?.entries, rangeEntries]
  )

  const handleSelectRange = useCallback(
    (anchorPath: string, currentPath: string) => {
      const range = buildSelectionRange(anchorPath, currentPath)
      setSelectedPaths(range)
    },
    [buildSelectionRange, setSelectedPaths]
  )

  const handleToggleEntrySelection = useCallback(
    (path: string, checked: boolean) => {
      setSelectedPaths((current) => {
        if (checked) {
          setActivePath(path)
          return current.includes(path) ? current : [...current, path]
        }

        return current.filter((item) => item !== path)
      })
    },
    [setActivePath, setSelectedPaths]
  )

  const handleClearSelection = useCallback(() => {
    setSelectedPaths([])
  }, [setSelectedPaths])

  return {
    activeEntry,
    breadcrumbs,
    contextMenuEntry,
    handleActivateEntry,
    handleClearSelection,
    handleSelectRange,
    handleToggleEntrySelection,
    selectedEntries,
  }
}
