import React from "react"
import type { ISearchResultChangeEvent } from "@xterm/addon-search"
import {
  ArrowDown,
  ArrowUp,
  CaseSensitive,
  GripHorizontal,
  Regex,
  WholeWord,
  X,
} from "lucide-react"
import type { TFunction } from "i18next"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import type { SearchOptionsState } from "@/components/TerminalTab/searchTypes"

type TerminalSearchBarProps = {
  isSearchDragging: boolean
  onClose: () => void
  onDragStart: (event: React.PointerEvent<HTMLDivElement>) => void
  onKeyDown: (event: React.KeyboardEvent<HTMLInputElement>) => void
  onRunSearch: (direction: "next" | "previous") => void
  onToggleOption: (option: keyof SearchOptionsState) => void
  searchInputRef: React.RefObject<HTMLInputElement>
  searchOptions: SearchOptionsState
  searchPosition: { x: number; y: number }
  searchQuery: string
  searchResultText: string
  searchResults: ISearchResultChangeEvent
  setSearchQuery: (value: string) => void
  t: TFunction
}

export const TerminalSearchBar: React.FC<TerminalSearchBarProps> = ({
  isSearchDragging,
  onClose,
  onDragStart,
  onKeyDown,
  onRunSearch,
  onToggleOption,
  searchInputRef,
  searchOptions,
  searchPosition,
  searchQuery,
  searchResultText,
  searchResults,
  setSearchQuery,
  t,
}) => (
  <div
    className="terminal-search-bar"
    data-dragging={isSearchDragging ? "true" : undefined}
    onMouseDown={(event) => event.stopPropagation()}
    style={{ transform: `translate(${searchPosition.x}px, ${searchPosition.y}px)` }}
  >
    <div
      className="terminal-search-drag-handle"
      onPointerDown={onDragStart}
      title={t("terminalSearch.drag", { defaultValue: "Drag search box" })}
      aria-label={t("terminalSearch.drag", { defaultValue: "Drag search box" })}
    >
      <GripHorizontal />
    </div>
    <Input
      ref={searchInputRef}
      value={searchQuery}
      onChange={(event) => setSearchQuery(event.target.value)}
      onKeyDown={onKeyDown}
      placeholder={t("terminalSearch.placeholder", { defaultValue: "Search terminal" })}
      aria-label={t("terminalSearch.placeholder", { defaultValue: "Search terminal" })}
      className="terminal-search-input"
    />
    <span
      className="terminal-search-count"
      aria-live="polite"
      data-empty={searchQuery ? undefined : "true"}
      data-missing={searchQuery && searchResults.resultCount === 0 ? "true" : undefined}
    >
      {searchResultText}
    </span>
    <Button
      type="button"
      variant={searchOptions.caseSensitive ? "secondary" : "ghost"}
      size="icon-xs"
      onClick={() => onToggleOption("caseSensitive")}
      title={t("terminalSearch.caseSensitive", { defaultValue: "Match case" })}
      aria-pressed={searchOptions.caseSensitive}
    >
      <CaseSensitive />
    </Button>
    <Button
      type="button"
      variant={searchOptions.wholeWord ? "secondary" : "ghost"}
      size="icon-xs"
      onClick={() => onToggleOption("wholeWord")}
      title={t("terminalSearch.wholeWord", { defaultValue: "Whole word" })}
      aria-pressed={searchOptions.wholeWord}
    >
      <WholeWord />
    </Button>
    <Button
      type="button"
      variant={searchOptions.regex ? "secondary" : "ghost"}
      size="icon-xs"
      onClick={() => onToggleOption("regex")}
      title={t("terminalSearch.regex", { defaultValue: "Use regular expression" })}
      aria-pressed={searchOptions.regex}
    >
      <Regex />
    </Button>
    <Button
      type="button"
      variant="ghost"
      size="icon-xs"
      onClick={() => onRunSearch("previous")}
      title={t("terminalSearch.previous", { defaultValue: "Previous result" })}
    >
      <ArrowUp />
    </Button>
    <Button
      type="button"
      variant="ghost"
      size="icon-xs"
      onClick={() => onRunSearch("next")}
      title={t("terminalSearch.next", { defaultValue: "Next result" })}
    >
      <ArrowDown />
    </Button>
    <Button
      type="button"
      variant="ghost"
      size="icon-xs"
      onClick={onClose}
      title={t("terminalSearch.close", { defaultValue: "Close search" })}
    >
      <X />
    </Button>
  </div>
)
