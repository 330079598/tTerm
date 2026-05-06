import { useCallback, useEffect, useRef, useState } from "react"
import type { SearchAddon, ISearchOptions, ISearchResultChangeEvent } from "@xterm/addon-search"
import type { Terminal } from "@xterm/xterm"

import {
  DEFAULT_SEARCH_OPTIONS,
  SEARCH_DECORATIONS,
  type SearchDragState,
  type SearchOptionsState,
} from "@/components/TerminalTab/searchTypes"

type UseTerminalSearchOptions = {
  isActiveRef: React.MutableRefObject<boolean>
  searchAddonRef: React.MutableRefObject<SearchAddon | null>
  setShowSftpDrawer: React.Dispatch<React.SetStateAction<boolean>>
  surfaceRef: React.RefObject<HTMLDivElement>
  termRef: React.MutableRefObject<Terminal | null>
}

type SearchDecorationDisposable = {
  dispose: () => void
}

export function useTerminalSearch({
  isActiveRef,
  searchAddonRef,
  setShowSftpDrawer,
  surfaceRef,
  termRef,
}: UseTerminalSearchOptions) {
  const searchInputRef = useRef<HTMLInputElement | null>(null)
  const searchDragStateRef = useRef<SearchDragState | null>(null)
  const activeSearchDecorationsRef = useRef<SearchDecorationDisposable[]>([])
  const previousSearchOptionsRef = useRef<SearchOptionsState>(DEFAULT_SEARCH_OPTIONS)
  const [isSearchOpen, setIsSearchOpen] = useState(false)
  const [searchQuery, setSearchQuery] = useState("")
  const [searchOptions, setSearchOptions] = useState<SearchOptionsState>(DEFAULT_SEARCH_OPTIONS)
  const [searchResults, setSearchResults] = useState<ISearchResultChangeEvent>({
    resultIndex: -1,
    resultCount: 0,
  })
  const [searchPosition, setSearchPosition] = useState({ x: 0, y: 0 })
  const [isSearchDragging, setIsSearchDragging] = useState(false)

  const buildSearchOptions = useCallback(
    (overrides?: Partial<SearchOptionsState>, incremental = false): ISearchOptions => ({
      ...searchOptions,
      ...overrides,
      incremental,
      decorations: SEARCH_DECORATIONS,
    }),
    [searchOptions]
  )

  const clearActiveSearchDecorations = useCallback(() => {
    for (const disposable of activeSearchDecorationsRef.current) {
      disposable.dispose()
    }
    activeSearchDecorationsRef.current = []
  }, [])

  const decorateActiveSearchSelection = useCallback(() => {
    const term = termRef.current
    const activeBackground = SEARCH_DECORATIONS.activeMatchBackground
    if (!term || !activeBackground) return

    const selection = term.getSelectionPosition()
    if (!selection) return

    clearActiveSearchDecorations()

    const activeBorder = SEARCH_DECORATIONS.activeMatchBorder
    const startRow = Math.min(selection.start.y, selection.end.y)
    const endRow = Math.max(selection.start.y, selection.end.y)

    for (let row = startRow; row <= endRow; row += 1) {
      const startCol = row === selection.start.y ? selection.start.x : 0
      const endCol = row === selection.end.y ? selection.end.x : term.cols
      const width = endCol - startCol
      if (width <= 0) continue

      const marker = term.registerMarker(
        row - term.buffer.active.baseY - term.buffer.active.cursorY
      )
      if (!marker) continue

      const decoration = term.registerDecoration({
        marker,
        x: startCol,
        width,
        backgroundColor: activeBackground,
        layer: "top",
      })

      if (!decoration) {
        marker.dispose()
        continue
      }

      const renderDisposable = decoration.onRender((element) => {
        element.classList.add("xterm-find-result-decoration", "xterm-find-active-result-decoration")
        if (activeBorder) {
          element.style.outline = `1px solid ${activeBorder}`
        }
      })

      activeSearchDecorationsRef.current.push({
        dispose: () => {
          renderDisposable.dispose()
          decoration.dispose()
          marker.dispose()
        },
      })
    }
  }, [clearActiveSearchDecorations, termRef])

  const runSearch = useCallback(
    (direction: "next" | "previous" = "next", incremental = false) => {
      const searchAddon = searchAddonRef.current
      const query = searchQuery
      if (!searchAddon) return false

      if (!query) {
        searchAddon.clearDecorations()
        setSearchResults({ resultIndex: -1, resultCount: 0 })
        return false
      }

      try {
        const options = buildSearchOptions(undefined, incremental)
        const found =
          direction === "previous"
            ? searchAddon.findPrevious(query, options)
            : searchAddon.findNext(query, options)

        if (found) {
          window.setTimeout(decorateActiveSearchSelection, 0)
        } else {
          clearActiveSearchDecorations()
        }

        return found
      } catch (error) {
        console.error("Terminal search failed:", error)
        clearActiveSearchDecorations()
        setSearchResults({ resultIndex: -1, resultCount: 0 })
        return false
      }
    },
    [
      buildSearchOptions,
      clearActiveSearchDecorations,
      decorateActiveSearchSelection,
      searchAddonRef,
      searchQuery,
    ]
  )

  const closeSearch = useCallback(() => {
    clearActiveSearchDecorations()
    searchAddonRef.current?.clearDecorations()
    setIsSearchOpen(false)
    setSearchQuery("")
    setSearchResults({ resultIndex: -1, resultCount: 0 })
    window.setTimeout(() => termRef.current?.focus(), 0)
  }, [clearActiveSearchDecorations, searchAddonRef, termRef])

  const openSearch = useCallback(() => {
    setIsSearchOpen(true)
    setShowSftpDrawer(false)
    window.setTimeout(() => {
      searchInputRef.current?.focus()
      searchInputRef.current?.select()
    }, 0)
  }, [setShowSftpDrawer])

  const handleSearchDragStart = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      if (event.button !== 0) return

      const bar = event.currentTarget.closest<HTMLDivElement>(".terminal-search-bar")
      const surface = surfaceRef.current
      if (!bar || !surface) return

      event.preventDefault()
      searchDragStateRef.current = {
        pointerX: event.clientX,
        pointerY: event.clientY,
        positionX: searchPosition.x,
        positionY: searchPosition.y,
        barRect: bar.getBoundingClientRect(),
        surfaceRect: surface.getBoundingClientRect(),
      }
      setIsSearchDragging(true)
    },
    [searchPosition.x, searchPosition.y, surfaceRef]
  )

  const toggleSearchOption = useCallback((option: keyof SearchOptionsState) => {
    setSearchOptions((current) => ({
      ...current,
      [option]: !current[option],
    }))
  }, [])

  useEffect(() => {
    if (!isSearchOpen) return

    window.setTimeout(() => {
      searchInputRef.current?.focus()
      searchInputRef.current?.select()
    }, 0)
  }, [isSearchOpen])

  useEffect(() => {
    if (!isSearchOpen) return

    const previousSearchOptions = previousSearchOptionsRef.current
    const didSearchOptionsChange =
      previousSearchOptions.caseSensitive !== searchOptions.caseSensitive ||
      previousSearchOptions.wholeWord !== searchOptions.wholeWord ||
      previousSearchOptions.regex !== searchOptions.regex
    previousSearchOptionsRef.current = searchOptions

    const searchTimer = window.setTimeout(() => {
      if (didSearchOptionsChange) {
        clearActiveSearchDecorations()
        searchAddonRef.current?.clearDecorations()
        setSearchResults({ resultIndex: -1, resultCount: 0 })
      }

      runSearch("next", !didSearchOptionsChange)
    }, 0)

    return () => window.clearTimeout(searchTimer)
  }, [clearActiveSearchDecorations, isSearchOpen, runSearch, searchAddonRef, searchOptions])

  useEffect(() => clearActiveSearchDecorations, [clearActiveSearchDecorations])

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (!isActiveRef.current) return

      if (isSearchOpen && event.key === "Escape") {
        event.preventDefault()
        closeSearch()
        return
      }

      const isFindShortcut = (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "f"
      if (!isFindShortcut) return

      event.preventDefault()
      openSearch()
    }

    window.addEventListener("keydown", handleKeyDown)
    return () => window.removeEventListener("keydown", handleKeyDown)
  }, [closeSearch, isActiveRef, isSearchOpen, openSearch])

  useEffect(() => {
    if (!isSearchDragging) return

    const margin = 8
    const handlePointerMove = (event: PointerEvent) => {
      const dragState = searchDragStateRef.current
      if (!dragState) return

      const deltaX = event.clientX - dragState.pointerX
      const deltaY = event.clientY - dragState.pointerY
      const minDeltaX = dragState.surfaceRect.left + margin - dragState.barRect.left
      const maxDeltaX = dragState.surfaceRect.right - margin - dragState.barRect.right
      const minDeltaY = dragState.surfaceRect.top + margin - dragState.barRect.top
      const maxDeltaY = dragState.surfaceRect.bottom - margin - dragState.barRect.bottom

      setSearchPosition({
        x: dragState.positionX + Math.min(Math.max(deltaX, minDeltaX), maxDeltaX),
        y: dragState.positionY + Math.min(Math.max(deltaY, minDeltaY), maxDeltaY),
      })
    }

    const handlePointerUp = () => {
      searchDragStateRef.current = null
      setIsSearchDragging(false)
    }

    window.addEventListener("pointermove", handlePointerMove)
    window.addEventListener("pointerup", handlePointerUp, { once: true })
    return () => {
      window.removeEventListener("pointermove", handlePointerMove)
      window.removeEventListener("pointerup", handlePointerUp)
    }
  }, [isSearchDragging])

  return {
    closeSearch,
    handleSearchDragStart,
    isSearchDragging,
    isSearchOpen,
    runSearch,
    searchInputRef,
    searchOptions,
    searchPosition,
    searchQuery,
    searchResults,
    setSearchQuery,
    setSearchResults,
    toggleSearchOption,
  }
}
