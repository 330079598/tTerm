import "@/i18n/config"
import "@/components/TerminalTab.css"

import { FitAddon } from "@xterm/addon-fit"
import { SearchAddon } from "@xterm/addon-search"
import { Terminal as Xterminal } from "@xterm/xterm"
import { useEffect, useRef } from "react"
import { useTranslation } from "react-i18next"

import { TerminalSearchBar } from "@/components/TerminalTab/TerminalSearchBar"
import { useTerminalSearch } from "@/components/TerminalTab/useTerminalSearch"
import { DEFAULT_TERMINAL_FONT_FAMILY, DEFAULT_TERMINAL_FONT_SIZE } from "@/lib/appConfig"
import { getPresetTheme } from "@/lib/themeDefinitions"
import type { ParityScreenProps } from "@/parity/types"

/** The synthetic output both captures use; no shell or SSH process is involved. */
const OUTPUT_LINES = 80
const SCROLL_UP_LINES = 30

function syntheticLine(index: number): string {
  return index % 6 === 0
    ? `\x1b[33mneedle\x1b[0m line-${String(index).padStart(3, "0")} 演示搜索\r\n`
    : `output line-${String(index).padStart(3, "0")} with a needle\r\n`
}

function rect(element: Element | null): DOMRect | null {
  return element ? element.getBoundingClientRect() : null
}

function rectJson(element: Element | null) {
  const bounds = rect(element)
  return bounds ? { x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height } : null
}

/**
 * The original search bar over a synthetic xterm terminal.
 *
 * This mounts the production `TerminalSearchBar`, the production
 * `useTerminalSearch` hook and the production stylesheet, so the reference is the
 * component pair the app ships rather than a redrawing of it. The connection
 * header, PTY and SSH session are intentionally absent: the terminal surface is
 * the full 800x600 content area the GPUI capture uses, and every byte is
 * synthetic output written in this process.
 */
export function TerminalSearchReference({ onReady }: ParityScreenProps) {
  return <SearchReferenceHarness onReady={onReady} />
}

function SearchReferenceHarness({ onReady }: ParityScreenProps) {
  const surfaceRef = useRef<HTMLDivElement | null>(null)
  const containerRef = useRef<HTMLDivElement | null>(null)
  const termRef = useRef<Xterminal | null>(null)
  const searchAddonRef = useRef<SearchAddon | null>(null)
  const isActiveRef = useRef(true)
  const { t } = useTranslation()

  const {
    closeSearch,
    handleSearchDragStart,
    isSearchDragging,
    isSearchOpen,
    openSearch,
    runSearch,
    searchInputRef,
    searchOptions,
    searchPosition,
    searchQuery,
    searchResults,
    setSearchQuery,
    setSearchResults,
    toggleSearchOption,
  } = useTerminalSearch({
    isActiveRef,
    searchAddonRef,
    setShowSftpDrawer: () => {},
    surfaceRef,
    termRef,
  })

  useEffect(() => {
    const surface = surfaceRef.current
    const container = containerRef.current
    if (!surface || !container) return

    const palette = getPresetTheme("default")?.terminal
    if (!palette) return

    const term = new Xterminal({
      scrollback: 2000,
      fontSize: DEFAULT_TERMINAL_FONT_SIZE,
      fontFamily: DEFAULT_TERMINAL_FONT_FAMILY,
      fontWeight: "normal",
      fontWeightBold: "bold",
      letterSpacing: 0,
      lineHeight: 1,
      theme: { ...palette, overviewRulerBorder: "#00000000" },
      allowProposedApi: true,
      disableStdin: true,
      cursorBlink: false,
      // The production bar draws its matches with the search addon's
      // decorations; the ruler is the same 10px track the app uses.
      overviewRuler: { width: 10 },
    })
    const fit = new FitAddon()
    const search = new SearchAddon({ highlightLimit: 2000 })
    term.loadAddon(fit)
    term.loadAddon(search)
    term.open(container)
    termRef.current = term
    searchAddonRef.current = search

    let cancelled = false
    void (async () => {
      await document.fonts.ready
      if (cancelled) return
      fit.fit()
      const output = Array.from({ length: OUTPUT_LINES }, (_, index) => syntheticLine(index)).join(
        ""
      )
      await new Promise<void>((resolve) => term.write(output, resolve))
      if (cancelled) return
      await new Promise<void>((resolve) => setTimeout(resolve, 100))
      if (cancelled) return
      // Match the GPUI capture's 30-line scroll from the bottom of the 80 rows.
      term.scrollToLine(term.buffer.active.baseY - SCROLL_UP_LINES)
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          if (!cancelled) onReady()
        })
      })
    })()

    const resultSubscription = search.onDidChangeResults((results) => {
      setSearchResults(results)
    })

    return () => {
      cancelled = true
      resultSubscription.dispose()
      term.dispose()
      termRef.current = null
      searchAddonRef.current = null
    }
  }, [onReady, setSearchResults])

  useEffect(() => {
    if (!isSearchOpen || searchResults.resultIndex < 0) return
    const term = termRef.current
    if (!term) return
    // The addon reveals a match by scrolling to it. Keep the fixture's 30-line
    // scroll and the query focus after every navigation so both captures
    // inspect the same viewport as the GPUI shot.
    term.scrollToLine(term.buffer.active.baseY - SCROLL_UP_LINES)
    searchInputRef.current?.focus()
  }, [isSearchOpen, searchInputRef, searchResults.resultIndex])

  useEffect(() => {
    window.__PARITY_STATE__ = () => {
      const term = termRef.current
      const surface = surfaceRef.current
      const bar = surface?.querySelector<HTMLElement>(".terminal-search-bar") ?? null
      const input = surface?.querySelector<HTMLInputElement>(".terminal-search-input") ?? null
      const handle = surface?.querySelector<HTMLElement>(".terminal-search-drag-handle") ?? null
      const count = surface?.querySelector<HTMLElement>(".terminal-search-count") ?? null
      const focused = document.activeElement
      return {
        method:
          "Playwright WebKit 26 + production TerminalSearchBar, useTerminalSearch and stylesheet + xterm.js SearchAddon; synthetic in-process output",
        shell: false,
        searchOpen: isSearchOpen,
        query: searchQuery,
        options: searchOptions,
        result: { index: searchResults.resultIndex, count: searchResults.resultCount },
        dragging: isSearchDragging,
        position: { x: searchPosition.x, y: searchPosition.y },
        focus:
          focused === input
            ? "search-input"
            : focused === handle
              ? "search-handle"
              : focused instanceof HTMLElement
                ? focused.tagName.toLowerCase()
                : null,
        terminal: term
          ? {
              cols: term.cols,
              rows: term.rows,
              viewportY: term.buffer.active.viewportY,
              baseY: term.buffer.active.baseY,
              cursorY: term.buffer.active.cursorY,
              selection: term.getSelectionPosition(),
              decorations: surface?.querySelectorAll(".xterm-find-result-decoration").length ?? 0,
              activeDecorations:
                surface?.querySelectorAll(".xterm-find-active-result-decoration").length ?? 0,
            }
          : null,
        bounds: {
          surface: rectJson(surface),
          bar: rectJson(bar),
          handle: rectJson(handle),
          input: rectJson(input),
          count: rectJson(count),
        },
        text: {
          count: count?.textContent ?? null,
          input: input?.value ?? null,
        },
        computed: bar
          ? (() => {
              const style = getComputedStyle(bar)
              return {
                position: style.position,
                top: style.top,
                right: style.right,
                transform: style.transform,
                width: style.width,
                height: style.height,
                gap: style.gap,
                padding: style.padding,
                borderRadius: style.borderRadius,
                borderWidth: style.borderTopWidth,
                backgroundColor: style.backgroundColor,
                boxShadow: style.boxShadow,
                backdropFilter: style.backdropFilter,
              }
            })()
          : null,
      }
    }
    return () => {
      delete window.__PARITY_STATE__
    }
  })

  return (
    <div
      ref={surfaceRef}
      className="terminal-surface"
      data-parity="terminal.search.surface"
      style={{ position: "fixed", inset: 0, padding: 0 }}
    >
      <div
        ref={containerRef}
        className="terminal-host"
        data-parity="terminal.search.terminal"
        style={{ position: "absolute", inset: 0, overflow: "hidden" }}
      />
      {isSearchOpen && (
        <TerminalSearchBar
          isSearchDragging={isSearchDragging}
          onClose={closeSearch}
          onDragStart={handleSearchDragStart}
          onRunSearch={(direction) => runSearch(direction)}
          onToggleOption={toggleSearchOption}
          searchInputRef={searchInputRef}
          searchOptions={searchOptions}
          searchPosition={searchPosition}
          searchQuery={searchQuery}
          searchResults={searchResults}
          setSearchQuery={setSearchQuery}
          t={t}
        />
      )}
      {!isSearchOpen && (
        <button
          type="button"
          data-parity-action="terminal.search.open"
          onClick={openSearch}
          style={{
            position: "absolute",
            top: 0,
            left: 0,
            width: 1,
            height: 1,
            opacity: 0,
            zIndex: 2,
            padding: 0,
            border: 0,
          }}
        >
          open
        </button>
      )}
    </div>
  )
}
